pub static IOSEVKA_FONT: &[u8] = include_bytes!("../compile/fonts/iosevka-extended.ttf");
pub static LOGO: &[u8] = include_bytes!("../compile/logo.png");

pub mod game;
pub mod render;
pub mod resource;
pub mod util;
