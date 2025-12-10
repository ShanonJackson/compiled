use super::{
    discard_comments::discard_comments, minify_gradients::minify_gradients,
    minify_params::minify_params, minify_selectors::minify_selectors,
    normalize_current_color::normalize_current_color, normalize_positions::normalize_positions,
    normalize_string::normalize_string, normalize_timing_functions::normalize_timing_functions,
    normalize_unicode::normalize_unicode, ordered_values::ordered_values,
};
use crate::postcss::transform::Plugin;
use crate::postcss::transform::TransformCssOptions;

/// Return the list of cssnano-aligned plugins that should be executed for normalization.
pub fn normalize_css(options: &TransformCssOptions) -> Vec<Box<dyn Plugin>> {
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    // cssnano-preset-default plugin order filtered to our allowlist (see
    // `packages/css/src/plugins/normalize-css.ts`).
    let optimize = options.optimize_css.unwrap_or(true);

    if optimize {
        plugins.push(Box::new(discard_comments()));
        plugins.push(Box::new(minify_gradients()));
        plugins.push(Box::new(super::reduce_initial::reduce_initial()));
        plugins.push(Box::new(super::colormin::colormin()));
        plugins.push(Box::new(normalize_timing_functions()));
        plugins.push(Box::new(super::calc::postcss_calc()));
        plugins.push(Box::new(super::convert_values::convert_values(false)));
        plugins.push(Box::new(ordered_values()));
    }

    // Base plugins always enabled (preserving cssnano ordering relative to the
    // filtered set above).
    plugins.push(Box::new(minify_selectors(true)));
    plugins.push(Box::new(minify_params()));

    if optimize {
        plugins.push(Box::new(normalize_string()));
        plugins.push(Box::new(normalize_unicode()));
        plugins.push(Box::new(super::normalize_url::normalize_url()));
        plugins.push(Box::new(normalize_positions()));

        // Custom plugin beyond cssnano preset default.
        plugins.push(Box::new(normalize_current_color()));
    }

    plugins
}
