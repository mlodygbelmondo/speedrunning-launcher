//! One file per screen (SCREENS.md numbering). Each adds `render_*` methods to `Scene`.
mod accounts;
mod check;
mod game;
mod home;
mod learn;
mod library;
mod modes;
mod onboarding;
mod practice;

pub use practice::PracticeSel;
mod settings;
mod signin;
mod stats;
mod submit;
mod tools;
