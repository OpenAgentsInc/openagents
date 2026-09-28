//! The ONNX hand landmarker.
//!
//! [`Landmarker::load`] opens the model file `model.toml` pins through
//! `ort`, and [`Landmarker::infer`] reads one RGB frame and answers the
//! hands in it. The camera reader is the caller's: `crates/coderos-camera`
//! hands it the frame it decoded for every consumer.

use crate::{Frame, Hand, Landmark, recognize};
use std::path::Path;

/// The side of the square the model reads.
pub const NET: u32 = 224;

/// The presence score under which a frame reports no hand.
const PRESENCE_FLOOR: f32 = 0.4;

/// One loaded model.
pub struct Landmarker {
    session: ort::session::Session,
    nchw: bool,
}

impl Landmarker {
    /// Loads the model at `path`. The error names the path.
    pub fn load(path: &Path) -> Result<Landmarker, String> {
        let session = ort::session::Session::builder()
            .map_err(|err| format!("onnx: {err}"))?
            .commit_from_file(path)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        let nchw = session
            .inputs()
            .first()
            .and_then(|i| i.dtype().tensor_shape().and_then(|d| d.get(1).copied()))
            .is_some_and(|d| d == 3);
        Ok(Landmarker { session, nchw })
    }

    /// The hands in one `width` by `height` RGB frame, three bytes a pixel.
    pub fn infer(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<Frame, String> {
        let small = resize_rgb(rgb, width, height, NET, NET);
        let tensor = tensor_from(&small, self.nchw);
        let shape: [usize; 4] = if self.nchw {
            [1, 3, NET as usize, NET as usize]
        } else {
            [1, NET as usize, NET as usize, 3]
        };
        let input = ort::value::Tensor::from_array((shape, tensor))
            .map_err(|err| format!("tensor: {err}"))?;
        let outputs = self
            .session
            .run(ort::inputs![input])
            .map_err(|err| format!("infer: {err}"))?;
        // The pinned model answers the 21 landmarks first and the presence
        // score as the second tensor; a model that packs the score after
        // the landmarks in one tensor is read the same way.
        let mut tensors = outputs.iter();
        let Some((_, first)) = tensors.next() else {
            return Ok(Frame {
                hands: Vec::new(),
                status: "model returned no tensors".into(),
            });
        };
        let (_shape, data) = first
            .try_extract_tensor::<f32>()
            .map_err(|err| format!("output: {err}"))?;
        let flat: Vec<f32> = data.to_vec();
        let second = tensors.next().and_then(|(_, second)| {
            second
                .try_extract_tensor::<f32>()
                .ok()
                .and_then(|(_, data)| data.first().copied())
        });
        Ok(read_landmarks(&flat, second))
    }
}

/// The frame one model answer describes: no hand when the answer is short
/// or the presence score is under the floor, one hand otherwise. The score
/// is the value packed after the 21 landmarks when the first tensor holds
/// one, and `second`, the first value of the next tensor, otherwise.
pub fn read_landmarks(flat: &[f32], second: Option<f32>) -> Frame {
    if flat.len() < 63 {
        return Frame {
            hands: Vec::new(),
            status: format!("model output len {}", flat.len()),
        };
    }
    let presence = flat.get(63).copied().or(second).unwrap_or(1.0);
    if presence < PRESENCE_FLOOR {
        return Frame {
            hands: Vec::new(),
            status: "No hands detected".into(),
        };
    }
    let mut landmarks = [Landmark::default(); 21];
    for (j, landmark) in landmarks.iter_mut().enumerate() {
        *landmark = Landmark {
            x: norm_coord(flat[j * 3]),
            y: norm_coord(flat[j * 3 + 1]),
            z: 1.0,
        };
    }
    let pose = recognize(&landmarks);
    Frame {
        hands: vec![Hand { landmarks, pose }],
        status: "1 hand(s) detected".into(),
    }
}

/// A coordinate in the frame, from a model that answers pixels or a
/// model that answers a fraction of the side.
fn norm_coord(v: f32) -> f32 {
    let t = if v.abs() > 1.5 { v / NET as f32 } else { v };
    t.clamp(0.0, 1.0)
}

/// The model's input from the resized square, in the layout it asks for.
fn tensor_from(small: &[u8], nchw: bool) -> Vec<f32> {
    let mut tensor = vec![0.0f32; (NET * NET * 3) as usize];
    if nchw {
        let plane = (NET * NET) as usize;
        for o in 0..plane {
            let i = o * 3;
            tensor[o] = small[i] as f32 / 255.0;
            tensor[plane + o] = small[i + 1] as f32 / 255.0;
            tensor[2 * plane + o] = small[i + 2] as f32 / 255.0;
        }
    } else {
        for (i, b) in small.iter().enumerate() {
            tensor[i] = *b as f32 / 255.0;
        }
    }
    tensor
}

/// A nearest-neighbour resize of an RGB frame.
pub fn resize_rgb(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dw * dh * 3) as usize];
    if sw == 0 || sh == 0 {
        return out;
    }
    for y in 0..dh {
        let sy = y * sh / dh;
        for x in 0..dw {
            let sx = x * sw / dw;
            let si = ((sy * sw + sx) * 3) as usize;
            let di = ((y * dw + x) * 3) as usize;
            if si + 2 < src.len() && di + 2 < out.len() {
                out[di..di + 3].copy_from_slice(&src[si..si + 3]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HandPose;

    #[test]
    fn a_short_answer_names_its_length() {
        let frame = read_landmarks(&[0.0; 10], None);
        assert!(frame.hands.is_empty());
        assert_eq!(frame.status, "model output len 10");
    }

    #[test]
    fn a_low_presence_score_is_no_hand() {
        let frame = read_landmarks(&[100.0; 63], Some(0.1));
        assert!(frame.hands.is_empty());
        assert_eq!(frame.status, "No hands detected");
    }

    #[test]
    fn pixels_are_normalized_to_the_side_and_a_pinch_is_read() {
        // The thumb and index tips touch and the wrist sits a palm below
        // the index knuckle, so the gap is nothing over a palm and the
        // pose is a pinch. The coordinate divides by the net's side.
        let mut flat = vec![112.0f32; 63];
        flat[crate::WRIST * 3 + 1] = 140.0;
        let frame = read_landmarks(&flat, Some(0.9));
        assert_eq!(frame.hands.len(), 1);
        assert_eq!(frame.hands[0].pose, HandPose::PinchClosed);
        assert!((frame.hands[0].landmarks[0].x - 0.5).abs() < 1e-3);
    }

    #[test]
    fn a_hand_the_net_collapsed_onto_one_pixel_is_no_pose() {
        // No palm to measure a pinch against, which is the reading the
        // pose rules refuse rather than call a pinch.
        let frame = read_landmarks(&vec![112.0f32; 63], Some(0.9));
        assert_eq!(frame.hands.len(), 1);
        assert_eq!(frame.hands[0].pose, HandPose::None);
    }

    #[test]
    fn a_score_packed_after_the_landmarks_is_read() {
        let mut flat = vec![0.5f32; 63];
        flat.push(0.0);
        let frame = read_landmarks(&flat, None);
        assert!(
            frame.hands.is_empty(),
            "the packed score is read as presence"
        );
    }

    #[test]
    fn resize_keeps_a_solid_colour_and_handles_an_empty_source() {
        let src = vec![7u8; 4 * 4 * 3];
        let out = resize_rgb(&src, 4, 4, 2, 2);
        assert_eq!(out, vec![7u8; 2 * 2 * 3]);
        assert_eq!(resize_rgb(&[], 0, 0, 2, 2), vec![0u8; 12]);
    }

    #[test]
    fn the_tensor_is_scaled_and_laid_out_by_plane_when_asked() {
        let mut small = vec![0u8; (NET * NET * 3) as usize];
        small[0] = 255;
        small[1] = 0;
        small[2] = 51;
        let nhwc = tensor_from(&small, false);
        assert!((nhwc[0] - 1.0).abs() < 1e-6);
        assert!((nhwc[2] - 0.2).abs() < 1e-6);
        let nchw = tensor_from(&small, true);
        assert!((nchw[0] - 1.0).abs() < 1e-6);
        assert!((nchw[2 * (NET * NET) as usize] - 0.2).abs() < 1e-6);
    }
}
