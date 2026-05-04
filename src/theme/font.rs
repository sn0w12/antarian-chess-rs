//! Built-in font families and runtime font registration.

use gpui::App;
use std::borrow::Cow;

macro_rules! embed_font_pairs {
    (
        $(
            $key:ident => (
                $ui_file:literal,
                $mono_file:literal,
                $ui_name:literal,
                $mono_name:literal
            )
        ),* $(,)?
    ) => {
        #[derive(Clone, Copy, Debug)]
        pub struct FontPair {
            pub ui: &'static str,
            pub mono: &'static str,
        }

        impl FontPair {
            $(
                pub const fn $key() -> Self {
                    Self {
                        ui: $ui_name,
                        mono: $mono_name,
                    }
                }
            )*
        }

        pub struct FontRegistry;

        static FONT_TABLE: &[(&str, FontPair)] = &[
            $(
                (stringify!($key), FontPair::$key()),
            )*
        ];

        impl FontRegistry {
            pub fn get(name: &str) -> Option<&'static FontPair> {
                FONT_TABLE.iter().find(|(k, _)| *k == name).map(|(_, v)| v)
            }
        }

        pub fn init_fonts(cx: &mut App) {
            $(
                let ui_data = include_bytes!(concat!("../assets/fonts/", $ui_file, ".ttf"));
                let mono_data = include_bytes!(concat!("../assets/fonts/", $mono_file, ".ttf"));

                if let Err(error) = cx.text_system().add_fonts(vec![
                    Cow::Borrowed(ui_data),
                    Cow::Borrowed(mono_data),
                ]) {
                    eprintln!("Failed to register bundled font pair {}: {error}", stringify!($key));
                }
            )*
        }
    };
}

// fn => (ui font file, mono font file, ui system name, mono system name)
embed_font_pairs!(
    geist => ("geist", "geist-mono", "Geist", "Geist Mono"),
);
