#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use crate::capi::*;

    #[test]
    fn test_app() {
        unsafe {
            let id = CString::new("saucers").unwrap();
            let opt = saucer_options_new(id.as_ptr());
            let app = saucer_application_init(opt);
            let prefs = saucer_preferences_new(app);
            let wv = saucer_new(prefs);
            let title_str = CString::new("Saucer").unwrap();
            saucer_window_set_title(wv, title_str.as_ptr());
            saucer_webview_set_dev_tools(wv, true);

            let url_str = CString::new("https://github.com").unwrap();
            saucer_webview_set_url(wv, url_str.as_ptr());
            saucer_window_set_size(wv, 1152, 648);
            saucer_window_show(wv);

            saucer_application_run(app);

            saucer_free(wv);
            saucer_preferences_free(prefs);
            saucer_application_free(app);
            saucer_options_free(opt);
        }
    }
}