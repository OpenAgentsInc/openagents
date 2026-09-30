//! Native image input. All file reading, decoding, and clipboard work runs off
//! the rendering thread; the application binds its result to a conversation.
use openagents_chat_app::attachments::{Image, MAX_IMAGE_BYTES};
use rust_native_desktop::input::PastedImage;
use std::{io::Read, path::PathBuf};
pub enum Source {
    Picker,
    Clipboard,
    File(PathBuf),
}
pub enum Result {
    Image(Image),
    Text(String),
    Cancelled,
    Failed(String),
}
pub fn read(source: Source) -> Result {
    match source {
        Source::Picker => {
            rust_native_desktop::input::pick_image().map_or(Result::Cancelled, read_file)
        }
        Source::File(path) => read_dropped(path),
        Source::Clipboard => match rust_native_desktop::input::paste_image() {
            Ok(Some(PastedImage::Pixels(image))) => {
                match (u32::try_from(image.width), u32::try_from(image.height)) {
                    (Ok(w), Ok(h)) => {
                        Image::pixels(w, h, image.rgba).map_or_else(Result::Failed, Result::Image)
                    }
                    _ => Result::Failed("The clipboard image is too large.".into()),
                }
            }
            Ok(Some(PastedImage::Encoded(bytes))) => {
                Image::decode("Pasted image", bytes).map_or_else(Result::Failed, Result::Image)
            }
            Ok(Some(PastedImage::File(path))) => read_dropped(path),
            Ok(None) => rust_native_desktop::input::paste().map_or(
                Result::Failed("The clipboard contains no image or text.".into()),
                Result::Text,
            ),
            Err(error) => {
                rust_native_desktop::input::paste().map_or(Result::Failed(error), Result::Text)
            }
        },
    }
}

/// Whether `path` names a file the composer attaches as an image, by its
/// extension: the formats [`Image::decode`] reads.
fn is_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["png", "jpg", "jpeg"]
                .iter()
                .any(|image| extension.eq_ignore_ascii_case(image))
        })
}

/// A dropped (or copied) file: an image is attached; any other file or a
/// folder puts its path in the message, as a terminal does, so Coder can
/// be pointed at it.
fn read_dropped(path: PathBuf) -> Result {
    dropped_path(&path).unwrap_or_else(|| read_file(path))
}

/// What a dropped file that is not an image puts in the message, at once
/// and without reading it; `None` for an image, which is read off the
/// window's thread.
pub fn dropped_path(path: &std::path::Path) -> Option<Result> {
    if is_image(path) {
        return None;
    }
    Some(match path.to_str() {
        Some(text) if path.exists() => Result::Text(format!("{text} ")),
        Some(_) => Result::Failed("That file is no longer there.".into()),
        None => Result::Failed("That file's name can't be put in a message.".into()),
    })
}
fn read_file(path: PathBuf) -> Result {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Image");
    if !path.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Result::Failed("Choose an image file.".into());
    }
    let Ok(file) = std::fs::File::open(&path) else {
        return Result::Failed("Couldn't open this image.".into());
    };
    let Ok(metadata) = file.metadata() else {
        return Result::Failed("Couldn't read this image.".into());
    };
    if !metadata.is_file() {
        return Result::Failed("Choose an image file.".into());
    }
    let mut bytes = vec![];
    if file
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Result::Failed("Couldn't read this image.".into());
    }
    Image::decode(name, bytes).map_or_else(Result::Failed, Result::Image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropped_and_picked_files_use_the_same_bounded_decoder() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sample.png");
        let image = Image::pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        std::fs::write(&path, image.bytes.as_slice()).unwrap();
        let Result::Image(read) = read(Source::File(path)) else {
            panic!("image")
        };
        assert_eq!(read.name, "sample.png");
        assert_eq!(read.preview.as_slice(), image.preview.as_slice());
        assert!(matches!(
            read_file(root.path().to_owned()),
            Result::Failed(_)
        ));
        let bad = root.path().join("bad.png");
        std::fs::write(&bad, b"not a png").unwrap();
        assert!(matches!(read_file(bad), Result::Failed(_)));
    }

    /// A dropped file that is not an image, or a folder, puts its path in
    /// the message; an image by any case of its extension is attached.
    #[test]
    fn dropped_files_that_are_not_images_become_their_paths() {
        let root = tempfile::tempdir().unwrap();
        let notes = root.path().join("notes.md");
        std::fs::write(&notes, b"# notes").unwrap();
        let Result::Text(text) = read(Source::File(notes.clone())) else {
            panic!("text")
        };
        assert_eq!(text, format!("{} ", notes.display()));
        let Result::Text(folder) = read(Source::File(root.path().to_owned())) else {
            panic!("folder")
        };
        assert_eq!(folder, format!("{} ", root.path().display()));
        assert!(matches!(
            read(Source::File(root.path().join("gone.txt"))),
            Result::Failed(_)
        ));
        let image = Image::pixels(1, 1, vec![1, 2, 3, 255]).unwrap();
        let shouting = root.path().join("SHOT.PNG");
        std::fs::write(&shouting, image.bytes.as_slice()).unwrap();
        assert!(matches!(read(Source::File(shouting)), Result::Image(_)));
    }
}
