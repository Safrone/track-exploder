# Releasing to Google Play

The GitHub release attaches a **sideload APK** signed with the Android debug
key. Play rejects debug-signed uploads, so the Play build is a separate,
manually-triggered workflow (`.github/workflows/android-aab.yml`) using its own
upload keystore.

## Before the first submission

Two things are worth knowing up front:

- **The Play build and the sideload APK have different signatures.** Anyone who
  installed the GitHub APK can't update in place from Play — they have to
  uninstall (losing app data) and reinstall. Enrolling in Play App Signing
  doesn't avoid this; Play re-signs with its own key either way.
- **Losing the upload key is recoverable but slow.** Play can reset it, but it
  takes a support round-trip. Back the keystore up somewhere durable and
  off-GitHub.

## One-time: create the upload keystore

Run locally, not in CI:

```bash
keytool -genkeypair -v \
  -keystore track-exploder-upload.jks \
  -alias track-exploder-upload \
  -keyalg RSA -keysize 4096 -validity 10000
```

Answer the prompts (the distinguished name is not user-visible) and pick a
password. Despite the `.jks` extension, `keytool` writes a **PKCS12** keystore
by default (since JDK 9), and PKCS12 can't hold a key password separate from the
store password — so you're only asked once, and `-keypass` is ignored if you
pass it. One password is correct here; there's nothing missing.

Then base64 the keystore for CI:

```bash
base64 -w0 track-exploder-upload.jks
```

Add four repository secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
| --- | --- |
| `ANDROID_UPLOAD_KEYSTORE_BASE64` | the base64 output above |
| `ANDROID_UPLOAD_KEY_ALIAS` | `track-exploder-upload` |
| `ANDROID_UPLOAD_STORE_PASSWORD` | the password |

(There's a fourth, `ANDROID_UPLOAD_KEY_PASSWORD`, but it's optional and only
applies to a legacy JKS-format keystore with a separate key password. Leave it
unset — the workflow falls back to the store password.)

Store the `.jks` and its password in a password manager. It is not in the
repo and must never be — `.gitignore` covers `*.jks`.

## Cutting a build

Actions → **Android AAB (Google Play)** → *Run workflow*, choosing the tag you
want to ship. The bundle lands under the run's **Artifacts** as
`Track-Exploder_<version>_<versionCode>.aab`; download it and upload it in Play
Console.

`versionCode` is derived from the app version in `src-tauri/tauri.conf.json`
(`major * 1000000 + minor * 1000 + patch`), so it increases on its own as long
as you bump the version. The workflow's optional `versionCode` input exists for
the one case that breaks: re-uploading after Play has already accepted that
number, where Play demands a higher one but the app version hasn't changed.

## Play Console checklist

Code isn't the whole submission. You'll also need a store listing (screenshots,
feature graphic, description), a privacy policy URL, a content rating
questionnaire, and the data safety form. Track Exploder processes audio locally
and doesn't collect or transmit user data, which makes the data safety form
short but still mandatory.
