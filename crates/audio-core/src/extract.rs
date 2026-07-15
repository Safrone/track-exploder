//! Extract the isolated voice from a "part-left" / "part-right" stereo track.
//!
//! In these learning tracks one part is hard-panned to a single channel while
//! the other three parts are summed on the opposite channel. Pulling out a clean
//! part is therefore just selecting the isolated channel.

use crate::decode::DecodedAudio;

/// Which stereo channel holds the isolated part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Left,
    Right,
}

impl Channel {
    fn index(self) -> usize {
        match self {
            Channel::Left => 0,
            Channel::Right => 1,
        }
    }
}

/// Return the isolated part as a mono `f32` buffer.
///
/// If the source is mono (or has fewer channels than requested), the first
/// channel is returned.
pub fn extract_channel(audio: &DecodedAudio, channel: Channel) -> Vec<f32> {
    if audio.planar.is_empty() {
        return Vec::new();
    }
    let idx = channel.index().min(audio.planar.len() - 1);
    audio.planar[idx].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(left: Vec<f32>, right: Vec<f32>) -> DecodedAudio {
        let frames = left.len().min(right.len());
        DecodedAudio {
            sample_rate: 44_100,
            channels: 2,
            frames,
            planar: vec![left, right],
        }
    }

    #[test]
    fn picks_left_channel() {
        let a = stereo(vec![0.1, 0.2, 0.3], vec![-0.1, -0.2, -0.3]);
        assert_eq!(extract_channel(&a, Channel::Left), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn picks_right_channel() {
        let a = stereo(vec![0.1, 0.2, 0.3], vec![-0.1, -0.2, -0.3]);
        assert_eq!(extract_channel(&a, Channel::Right), vec![-0.1, -0.2, -0.3]);
    }

    #[test]
    fn mono_falls_back_to_channel_zero() {
        let mono = DecodedAudio {
            sample_rate: 44_100,
            channels: 1,
            frames: 3,
            planar: vec![vec![1.0, 2.0, 3.0]],
        };
        assert_eq!(extract_channel(&mono, Channel::Right), vec![1.0, 2.0, 3.0]);
    }
}
