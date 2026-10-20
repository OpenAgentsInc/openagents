//! Image token counts for DeepSeek V4.1.

use std::fmt;

/// Result of fitting one image into the token budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeResult {
    /// Number of token rows in the model token grid.
    pub n_llm_h: usize,
    /// Number of token columns in the model token grid.
    pub n_llm_w: usize,
    /// Resized image height in pixels.
    pub best_height: usize,
    /// Resized image width in pixels.
    pub best_width: usize,
    /// Image token count: the number of tokens the resized image occupies in
    /// the inference input.
    pub token_count: usize,
}

/// Failure to fit an image into the token budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcResizeError {
    /// Fitting the image did not reach a fixed point within the iteration limit.
    NotConverged {
        max_iter: usize,
        last_result: ResizeResult,
    },
}

impl fmt::Display for CalcResizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConverged {
                max_iter,
                last_result,
            } => write!(
                f,
                "image resize did not converge after {max_iter} iterations, last result: {last_result:?}"
            ),
        }
    }
}

impl std::error::Error for CalcResizeError {}

/// DeepSeek V4.1 image token specification.
///
/// The specification maps an image size to its image token count. V4.1 uses
/// 14-pixel patches and a spatial downsample ratio of 3.
/// Images with a positive area below 544 * 544 pixels are upscaled before
/// patch alignment and token-budget fitting. This is an area target, not a
/// minimum for each dimension; token-budget fitting can reduce it further.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageTokenSpec {
    patch_size: usize,
    downsample_ratio: usize,
    max_token_count: usize,
    min_pixels: usize,
}

impl ImageTokenSpec {
    /// Return the DeepSeek V4.1 image token specification.
    ///
    /// Images are divided into 14-pixel patches, downsampled by 3 in each
    /// dimension, and limited to an image token count of 1024. Images with a
    /// positive area below 544 * 544 pixels are upscaled before alignment and
    /// budget fitting.
    pub const fn v41() -> Self {
        Self {
            patch_size: 14,
            downsample_ratio: 3,
            max_token_count: 1024,
            min_pixels: 544 * 544,
        }
    }

    /// Return the maximum image token count of one image.
    pub const fn max_token_count(&self) -> usize {
        self.max_token_count
    }

    /// Return the side length of one square patch in pixels.
    pub const fn patch_size(&self) -> usize {
        self.patch_size
    }

    /// Return the spatial downsampling ratio between the patch grid and the
    /// model token grid.
    pub const fn downsample_ratio(&self) -> usize {
        self.downsample_ratio
    }

    /// Return the image token count of an image of `width`×`height` pixels:
    /// the number of tokens it occupies in the inference input.
    ///
    /// # Errors
    ///
    /// Returns [`CalcResizeError`] when fitting the image does not converge.
    pub fn calc_token_count(&self, width: usize, height: usize) -> Result<usize, CalcResizeError> {
        self.calc_resize(width, height)
            .map(|result| result.token_count)
    }

    /// Return whether token-budget fitting leaves `width`×`height` unchanged.
    /// This checks dimensions only; it does not inspect encoded image bytes.
    /// Returns `false` when fitting does not converge.
    pub fn is_image_valid(&self, width: usize, height: usize) -> bool {
        self.calc_resize(width, height)
            .map(|result| result.best_width == width && result.best_height == height)
            .unwrap_or(false)
    }

    /// Return the target size and image token count for an image of
    /// `width`×`height` pixels.
    ///
    /// # Errors
    ///
    /// Returns [`CalcResizeError`] when fitting the image does not converge.
    pub fn calc_resize(
        &self,
        width: usize,
        height: usize,
    ) -> Result<ResizeResult, CalcResizeError> {
        const MAX_ITER: usize = 10;
        let mut result = self.calc_resize_once(width, height);
        for _ in 1..MAX_ITER {
            let next = self.calc_resize_once(result.best_width, result.best_height);
            if next == result {
                return Ok(result);
            }
            result = next;
        }
        Err(CalcResizeError::NotConverged {
            max_iter: MAX_ITER,
            last_result: result,
        })
    }

    /// Upscale to the minimum area, align to patches, and fit the token budget.
    fn calc_resize_once(&self, width: usize, height: usize) -> ResizeResult {
        let mut width = width;
        let mut height = height;

        let current_pixels = width * height;
        if current_pixels < self.min_pixels && current_pixels > 0 {
            let ratio = (self.min_pixels as f64 / current_pixels as f64).sqrt();
            width = (width as f64 * ratio) as usize;
            height = (height as f64 * ratio) as usize;
        }

        let best_width = width.div_ceil(self.patch_size) * self.patch_size;
        let best_height = height.div_ceil(self.patch_size) * self.patch_size;
        self.fit_token_budget(height, width, best_height, best_width)
    }

    /// Return the aligned size `best_width`×`best_height` if it fits the token
    /// budget; otherwise solve once for the largest aspect-preserving size
    /// within the budget.
    fn fit_token_budget(
        &self,
        height: usize,
        width: usize,
        best_height: usize,
        best_width: usize,
    ) -> ResizeResult {
        let mut result = self.resize_result(best_height, best_width);
        if result.token_count > self.max_token_count {
            result = self.solve_resize_ratio(height, width, self.max_token_count);
            assert!(
                result.token_count <= self.max_token_count,
                "token budget must be satisfied after one solve"
            );
        }
        result
    }

    /// Return the token grid and image token count of a
    /// `best_width`×`best_height` image that is already aligned to patches.
    fn resize_result(&self, best_height: usize, best_width: usize) -> ResizeResult {
        let n_llm_h = (best_height / self.patch_size).div_ceil(self.downsample_ratio);
        let n_llm_w = (best_width / self.patch_size).div_ceil(self.downsample_ratio);
        ResizeResult {
            n_llm_h,
            n_llm_w,
            best_height,
            best_width,
            token_count: self.calc_grid_token_count(n_llm_h, n_llm_w),
        }
    }

    /// Return the image token count of a token grid with `n_llm_h` rows and
    /// `n_llm_w` columns.
    ///
    /// Each row holds one token per column and one newline token, and a start
    /// token and an end token delimit the image.
    fn calc_grid_token_count(&self, n_llm_h: usize, n_llm_w: usize) -> usize {
        n_llm_h * (n_llm_w + 1) + 2
    }

    /// Solve for the largest aspect-preserving size within `max_token_count`.
    fn solve_resize_ratio(
        &self,
        height: usize,
        width: usize,
        max_token_count: usize,
    ) -> ResizeResult {
        let ratio = height as f64 / width as f64;
        let max_w_float = ((max_token_count as f64 - 2.0) / ratio + 0.25).sqrt() - 0.5;
        let max_h_float = max_w_float * ratio;

        let (best_height, best_width);
        if max_w_float < 1.0 {
            let max_w = 1;
            let max_h = (max_token_count - 2) / (max_w + 1);
            best_width = max_w * self.patch_size * self.downsample_ratio;
            best_height = max_h * self.patch_size * self.downsample_ratio;
        } else if max_h_float < 1.0 {
            let max_h = 1;
            let max_w = (max_token_count - 2) / max_h - 1;
            assert!(max_w > 1, "token budget must allow a two-column grid");
            best_width = max_w * self.patch_size * self.downsample_ratio;
            best_height = max_h * self.patch_size * self.downsample_ratio;
        } else {
            let max_w = max_w_float as usize;
            let max_h = max_h_float as usize;
            let beta_w = (max_w * self.patch_size * self.downsample_ratio) as f64 / width as f64;
            let beta_h = (max_h * self.patch_size * self.downsample_ratio) as f64 / height as f64;
            let beta = beta_w.min(beta_h);
            best_width = (width as f64 * beta / self.patch_size as f64) as usize * self.patch_size;
            best_height =
                (height as f64 * beta / self.patch_size as f64) as usize * self.patch_size;
        }
        self.resize_result(best_height, best_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected target sizes and token counts of the V4.1 preprocessing.
    const CASES: &[(usize, usize, usize, usize, usize, usize, usize)] = &[
        // width, height, token_count, best_width, best_height, n_llm_h, n_llm_w
        (978, 479, 302, 980, 490, 12, 24),
        (272, 300, 198, 518, 574, 14, 13),
        (516, 517, 184, 546, 546, 13, 13),
        (1091, 397, 272, 1092, 406, 10, 26),
        (743, 870, 401, 756, 882, 21, 18),
        (906, 740, 416, 910, 742, 18, 22),
        (1107, 873, 590, 1120, 882, 21, 27),
        (493, 243, 202, 784, 392, 10, 19),
        (354, 476, 197, 476, 630, 15, 12),
        (766, 642, 322, 770, 644, 16, 19),
        (972, 672, 402, 980, 672, 16, 24),
        (1167, 486, 350, 1176, 490, 12, 28),
        (1134, 389, 282, 1134, 392, 10, 27),
        (275, 346, 197, 490, 616, 15, 12),
        (1033, 141, 187, 1484, 210, 5, 36),
        (125, 751, 226, 224, 1344, 32, 6),
        (560, 1029, 377, 560, 1036, 25, 14),
        (482, 725, 236, 490, 728, 18, 12),
        (782, 879, 422, 784, 882, 21, 19),
        (434, 272, 200, 700, 434, 11, 17),
        (2, 165, 356, 70, 4942, 118, 2),
        (165, 2, 240, 4942, 70, 2, 118),
        (158, 10000, 762, 126, 7966, 190, 3),
        (10000, 158, 962, 10010, 168, 4, 239),
        (40, 50, 197, 490, 616, 15, 12),
        (50, 40, 194, 616, 490, 12, 15),
        (1, 1, 184, 546, 546, 13, 13),
        (100, 100, 184, 546, 546, 13, 13),
        (16809, 11841, 990, 1540, 1092, 26, 37),
        (1920, 330, 378, 1932, 336, 8, 46),
        (8192, 8192, 994, 1302, 1302, 31, 31),
        (4096, 4096, 994, 1302, 1302, 31, 31),
        (1920, 1080, 968, 1708, 966, 23, 41),
        (1080, 1920, 986, 966, 1708, 41, 23),
        (3024, 4032, 1010, 1134, 1512, 36, 27),
        (640, 480, 206, 644, 490, 12, 16),
    ];

    #[test]
    fn calc_resize_matches_v41_preprocessing() {
        let spec = ImageTokenSpec::v41();
        for &(width, height, token_count, best_width, best_height, n_llm_h, n_llm_w) in CASES {
            let result = spec.calc_resize(width, height).expect("resize converges");
            assert_eq!(
                result,
                ResizeResult {
                    n_llm_h,
                    n_llm_w,
                    best_height,
                    best_width,
                    token_count,
                },
                "calc_resize({width}, {height})"
            );
            assert!(
                result.token_count <= spec.max_token_count(),
                "({width}, {height}) exceeds the budget"
            );
        }
    }

    #[test]
    fn fitted_images_are_valid() {
        let spec = ImageTokenSpec::v41();
        for &(_, _, _, best_width, best_height, _, _) in CASES {
            assert!(
                spec.is_image_valid(best_width, best_height),
                "{best_width}x{best_height}"
            );
        }
    }

    #[test]
    fn token_count_has_a_floor_and_a_ceiling() {
        let spec = ImageTokenSpec::v41();
        assert_eq!(spec.calc_token_count(1, 1).expect("resize converges"), 184);
        assert_eq!(
            spec.calc_token_count(16809, 11841)
                .expect("resize converges"),
            990
        );
    }
}
