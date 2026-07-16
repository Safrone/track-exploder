//! Read tags from input files and write the common ones into an exported file.
//!
//! Reading uses Symphonia so it works uniformly across MP3/FLAC/WAV/M4A inputs.
//! Writing uses format-specific taggers (`id3` for MP3, `metaflac` for FLAC).

use std::collections::BTreeMap;
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Tag};
use symphonia::core::probe::Hint;
use thiserror::Error;

use crate::decode::DecodeError;
use crate::encode::ExportFormat;

/// A normalized set of tags (canonical lower-case keys → values).
pub type Tags = BTreeMap<String, String>;

#[derive(Debug, Error)]
pub enum TagError {
    #[error("flac tag error: {0}")]
    Flac(String),
    #[error("id3 tag error: {0}")]
    Id3(String),
}

/// Map a Symphonia standard tag key to our small canonical set (or None to skip).
fn canonical(key: StandardTagKey) -> Option<&'static str> {
    use StandardTagKey::*;
    Some(match key {
        TrackTitle => "title",
        Artist => "artist",
        Album => "album",
        AlbumArtist => "albumartist",
        Date | ReleaseDate | OriginalDate => "date",
        Genre => "genre",
        Composer => "composer",
        Comment => "comment",
        _ => return None,
    })
}

fn collect(tags: &[Tag], out: &mut Tags) {
    for t in tags {
        if let Some(std) = t.std_key {
            if let Some(k) = canonical(std) {
                let value = t.value.to_string();
                if !value.is_empty() {
                    out.entry(k.to_string()).or_insert(value);
                }
            }
        }
    }
}

/// Read a normalized set of tags from any supported audio file.
pub fn read_tags(path: &Path) -> Result<Tags, DecodeError> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    read_tags_mss(mss, hint)
}

/// Read a normalized set of tags from in-memory audio bytes.
pub fn read_tags_bytes(bytes: Vec<u8>, ext_hint: Option<&str>) -> Result<Tags, DecodeError> {
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = ext_hint {
        hint.with_extension(ext);
    }
    read_tags_mss(mss, hint)
}

fn read_tags_mss(mss: MediaSourceStream, hint: Hint) -> Result<Tags, DecodeError> {
    let mut probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut tags = Tags::new();
    // Container-level metadata read during probing (e.g. ID3v2 ahead of MP3 frames).
    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        collect(rev.tags(), &mut tags);
    }
    // Metadata surfaced by the format reader itself.
    if let Some(rev) = probed.format.metadata().current() {
        collect(rev.tags(), &mut tags);
    }
    Ok(tags)
}

/// Write `tags` into the file at `path`, using the appropriate tagger for `format`.
/// A no-op for empty tags or WAV (RIFF INFO tags are not yet supported).
pub fn write_tags(path: &Path, format: ExportFormat, tags: &Tags) -> Result<(), TagError> {
    if tags.is_empty() {
        return Ok(());
    }
    match format {
        ExportFormat::Wav => Ok(()),
        ExportFormat::Flac => write_flac_tags(path, tags),
        #[cfg(feature = "mp3")]
        ExportFormat::Mp3 => write_mp3_tags(path, tags),
    }
}

fn write_flac_tags(path: &Path, tags: &Tags) -> Result<(), TagError> {
    let mut tag = metaflac::Tag::read_from_path(path).map_err(|e| TagError::Flac(e.to_string()))?;
    {
        let vc = tag.vorbis_comments_mut();
        if let Some(v) = tags.get("title") {
            vc.set_title(vec![v.clone()]);
        }
        if let Some(v) = tags.get("artist") {
            vc.set_artist(vec![v.clone()]);
        }
        if let Some(v) = tags.get("album") {
            vc.set_album(vec![v.clone()]);
        }
        if let Some(v) = tags.get("albumartist") {
            vc.set_album_artist(vec![v.clone()]);
        }
        if let Some(v) = tags.get("genre") {
            vc.set_genre(vec![v.clone()]);
        }
        if let Some(v) = tags.get("date") {
            vc.set("DATE", vec![v.clone()]);
        }
        if let Some(v) = tags.get("composer") {
            vc.set("COMPOSER", vec![v.clone()]);
        }
        if let Some(v) = tags.get("comment") {
            vc.set("COMMENT", vec![v.clone()]);
        }
    }
    tag.save().map_err(|e| TagError::Flac(e.to_string()))
}

#[cfg(feature = "mp3")]
fn write_mp3_tags(path: &Path, tags: &Tags) -> Result<(), TagError> {
    use id3::{TagLike, Version};

    let mut tag = id3::Tag::new();
    if let Some(v) = tags.get("title") {
        tag.set_title(v.clone());
    }
    if let Some(v) = tags.get("artist") {
        tag.set_artist(v.clone());
    }
    if let Some(v) = tags.get("album") {
        tag.set_album(v.clone());
    }
    if let Some(v) = tags.get("albumartist") {
        tag.set_album_artist(v.clone());
    }
    if let Some(v) = tags.get("genre") {
        tag.set_genre(v.clone());
    }
    if let Some(y) = tags
        .get("date")
        .and_then(|d| d.get(0..4))
        .and_then(|y| y.parse().ok())
    {
        tag.set_year(y);
    }
    tag.write_to_path(path, Version::Id3v24)
        .map_err(|e| TagError::Id3(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_tags_is_noop_for_empty() {
        let tags = Tags::new();
        // Path need not exist because empty tags short-circuit.
        assert!(write_tags(Path::new("/nonexistent"), ExportFormat::Flac, &tags).is_ok());
    }

    #[test]
    fn flac_tag_roundtrip() {
        use crate::encode::{encode_interleaved, BitDepth};

        let samples: Vec<f32> = (0..4096).map(|i| ((i as f32) * 0.01).sin() * 0.4).collect();
        let bytes =
            encode_interleaved(&samples, 1, 44_100, ExportFormat::Flac, BitDepth::Sixteen).unwrap();

        let mut path = std::env::temp_dir();
        path.push(format!("te_tagtest_{}.flac", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();

        let mut tags = Tags::new();
        tags.insert("album".into(), "Brigade".into());
        tags.insert("title".into(), "Test".into());
        write_tags(&path, ExportFormat::Flac, &tags).unwrap();

        let read = read_tags(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(read.get("album").map(String::as_str), Some("Brigade"));
        assert_eq!(read.get("title").map(String::as_str), Some("Test"));
    }
}
