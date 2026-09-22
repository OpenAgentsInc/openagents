//! NIP-68 picture-first events.
//!
//! A kind `20` event is a picture post: a `title` tag and the content
//! description, with each image in an `imeta` tag whose MIME type is
//! one of the accepted image formats. `x` and `m` tags repeat each
//! image's hash and type for filtering, `annotate-user` fields place
//! profile links in a picture, and `location`, `g`, `l`, `p`, and
//! `t` tags carry their usual meanings. NIP-68 is a draft, so the
//! kind is not added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::imeta::{Imeta, open_imetas};
use super::{DomainError, Event};

const PICTURE_KIND: u16 = 20;

/// The image MIME types a kind `20` may carry.
pub const PICTURE_MEDIA_TYPES: &[&str] = &[
    "image/apng",
    "image/avif",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
];

/// A user annotation an `annotate-user` imeta field carries.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedUser {
    /// The tagged pubkey.
    pub pubkey: String,
    /// Horizontal position in the image.
    pub x: f64,
    /// Vertical position in the image.
    pub y: f64,
}

/// A validated kind `20` picture post.
#[derive(Clone, Debug)]
pub struct Picture {
    /// The `title` tag.
    pub title: String,
    /// The post description — the event content.
    pub description: String,
    /// The post's images, one `imeta` each.
    pub images: Vec<Imeta>,
}

/// Read a kind `20` event into a picture post: a `title` and at least
/// one `imeta` whose `m` is an accepted image type.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// empty `title`, a malformed `imeta`, or no accepted image.
pub fn open_picture(event: &Event) -> Result<Picture, DomainError> {
    if event.kind != PICTURE_KIND {
        return Err(invalid("a picture post is kind 20"));
    }
    let title = event
        .tag_values("title")
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("a picture post requires a title tag"))?
        .to_string();
    let images: Vec<Imeta> = open_imetas(event)?
        .into_iter()
        .filter(|imeta| {
            imeta
                .media_type()
                .is_some_and(|m| PICTURE_MEDIA_TYPES.contains(&m))
        })
        .collect();
    if images.is_empty() {
        return Err(invalid(
            "a picture post needs an imeta image of an accepted type",
        ));
    }
    for imeta in &images {
        for field in imeta.all("annotate-user") {
            annotated_user(field)?;
        }
    }
    Ok(Picture {
        title,
        description: event.content.clone(),
        images,
    })
}

/// Parse an `annotate-user` field: `<pubkey>:<posX>:<posY>`.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for a malformed entry.
pub fn annotated_user(field: &str) -> Result<AnnotatedUser, DomainError> {
    let parts: Vec<&str> = field.split(':').collect();
    let [pubkey, x, y] = parts.as_slice() else {
        return Err(invalid("annotate-user is pubkey:posX:posY"));
    };
    decode_lower_hex::<32>(pubkey, "annotate-user")
        .map_err(|_| invalid("annotate-user names a pubkey"))?;
    Ok(AnnotatedUser {
        pubkey: (*pubkey).to_string(),
        x: x.parse()
            .map_err(|_| invalid("annotate-user posX is a number"))?,
        y: y.parse()
            .map_err(|_| invalid("annotate-user posY is a number"))?,
    })
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn a_picture_post_carries_its_images_in_imeta_tags() {
        let post = sign(
            PICTURE_KIND,
            vec![
                Tag::new(vec!["title".into(), "Coast trip".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://nostr.build/i/one.jpg".into(),
                    "m image/jpeg".into(),
                    "dim 3024x4032".into(),
                    format!("x {}", "cd".repeat(32)),
                ]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://nostr.build/i/two.png".into(),
                    "m image/png".into(),
                    format!("annotate-user {}:50:25", "ab".repeat(32)),
                ]),
                Tag::new(vec!["m".into(), "image/jpeg".into()]),
                Tag::new(vec!["x".into(), "cd".repeat(32)]),
                Tag::new(vec!["t".into(), "travel".into()]),
            ],
            "Two shots from the coast",
        );
        let picture = open_picture(&post).unwrap();
        assert_eq!(picture.title, "Coast trip");
        assert_eq!(picture.images.len(), 2);
        let annotation = annotated_user(picture.images[1].all("annotate-user")[0]).unwrap();
        assert_eq!(annotation.pubkey, "ab".repeat(32));
        assert_eq!((annotation.x, annotation.y), (50.0, 25.0));
    }

    #[test]
    fn malformed_picture_posts_are_refused() {
        let no_title = sign(
            PICTURE_KIND,
            vec![Tag::new(vec![
                "imeta".into(),
                "url https://x/i.png".into(),
                "m image/png".into(),
            ])],
            "x",
        );
        assert!(open_picture(&no_title).is_err());
        let video_only = sign(
            PICTURE_KIND,
            vec![
                Tag::new(vec!["title".into(), "t".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://x/v.mp4".into(),
                    "m video/mp4".into(),
                ]),
            ],
            "x",
        );
        assert!(open_picture(&video_only).is_err());
        let wrong_kind = sign(1, vec![Tag::new(vec!["title".into(), "t".into()])], "note");
        assert!(open_picture(&wrong_kind).is_err());
        assert!(annotated_user("not:a:user").is_err());
        assert!(annotated_user(&format!("{}:x:y", "ab".repeat(32))).is_err());
    }
}
