//! Zoom / fit model, ported from `mcomix/zoom.py`.

pub const ZOOM_MODE_BEST: i32 = 0;
pub const ZOOM_MODE_WIDTH: i32 = 1;
pub const ZOOM_MODE_HEIGHT: i32 = 2;
pub const ZOOM_MODE_MANUAL: i32 = 3;
pub const ZOOM_MODE_SIZE: i32 = 4;

const IDENTITY_ZOOM_LOG: i32 = 0;
const MIN_USER_ZOOM_LOG: i32 = -20;
const MAX_USER_ZOOM_LOG: i32 = 12;
const USER_ZOOM_LOG_SCALE1: f64 = 4.0;

#[derive(Debug, Clone)]
pub struct ZoomModel {
    /// Fit mode, one of `ZOOM_MODE_*`.
    pub fit_mode: i32,
    /// User zoom offset in logarithmic steps; `scale = 2^(log/4)`.
    pub user_zoom_log: i32,
    /// Whether fitting may upscale images beyond 100 %.
    pub scale_up: bool,
}

impl Default for ZoomModel {
    fn default() -> Self {
        ZoomModel {
            fit_mode: ZOOM_MODE_BEST,
            user_zoom_log: IDENTITY_ZOOM_LOG,
            scale_up: false,
        }
    }
}

impl ZoomModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_fit_mode(&mut self, mode: i32) {
        if (ZOOM_MODE_BEST..=ZOOM_MODE_SIZE).contains(&mode) {
            self.fit_mode = mode;
        }
    }

    pub fn zoom_in(&mut self) {
        self.user_zoom_log = (self.user_zoom_log + 1).min(MAX_USER_ZOOM_LOG);
    }

    pub fn zoom_out(&mut self) {
        self.user_zoom_log = (self.user_zoom_log - 1).max(MIN_USER_ZOOM_LOG);
    }

    pub fn reset_user_zoom(&mut self) {
        self.user_zoom_log = IDENTITY_ZOOM_LOG;
    }

    pub fn user_scale(&self) -> f64 {
        2f64.powf(self.user_zoom_log as f64 / USER_ZOOM_LOG_SCALE1)
    }

    /// Compute the on-screen size of each visible page.
    ///
    /// `image_sizes` are the full-resolution (rotated) sizes of the pages on
    /// the current spread, `viewport` is the available drawing area,
    /// `spacing` the gap between pages in double-page mode.
    pub fn zoomed_sizes(
        &self,
        image_sizes: &[(u32, u32)],
        viewport: (f64, f64),
        spacing: f64,
        double_page: bool,
        fit_to_size_mode: i32,
        fit_to_size_px: u32,
    ) -> Vec<(u32, u32)> {
        if image_sizes.is_empty() {
            return Vec::new();
        }
        let (sw, sh) = viewport;
        let sw = if sw > 0.0 { sw } else { 1.0 };
        let sh = if sh > 0.0 { sh } else { 1.0 };

        // Union size of the spread (pages distributed along the width axis).
        let uw = if double_page {
            image_sizes.iter().map(|s| s.0 as f64).sum::<f64>() + spacing
        } else {
            image_sizes[0].0 as f64
        };
        let uh = image_sizes
            .iter()
            .map(|s| s.1 as f64)
            .fold(0.0_f64, f64::max);

        // Limits per axis, mirroring ZoomModel._calc_limits.
        let resolved = if self.fit_mode == ZOOM_MODE_SIZE {
            fit_to_size_mode
        } else {
            self.fit_mode
        };
        let fixed = fit_to_size_px as f64;
        let (limit_w, limit_h): (Option<f64>, Option<f64>) = match self.fit_mode {
            ZOOM_MODE_MANUAL => (None, None),
            ZOOM_MODE_WIDTH => (Some(sw), None),
            ZOOM_MODE_HEIGHT => (None, Some(sh)),
            ZOOM_MODE_BEST => (Some(sw), Some(sh)),
            ZOOM_MODE_SIZE => match resolved {
                ZOOM_MODE_WIDTH => (Some(fixed), None),
                ZOOM_MODE_HEIGHT => (None, Some(fixed)),
                ZOOM_MODE_BEST => (Some(fixed), Some(fixed)),
                _ => (Some(fixed), Some(fixed)),
            },
            _ => (Some(sw), Some(sh)),
        };

        let mut scale: f64 = 1.0;
        if let Some(lw) = limit_w {
            scale = scale.min(lw / uw);
        }
        if let Some(lh) = limit_h {
            scale = scale.min(lh / uh);
        }
        // Do not upscale small images unless the user zooms in.
        if !self.scale_up && self.fit_mode != ZOOM_MODE_MANUAL {
            scale = scale.min(1.0);
        }
        scale *= self.user_scale();

        image_sizes
            .iter()
            .map(|(w, h)| {
                (
                    ((*w as f64) * scale).round().max(1.0) as u32,
                    ((*h as f64) * scale).round().max(1.0) as u32,
                )
            })
            .collect()
    }
}
