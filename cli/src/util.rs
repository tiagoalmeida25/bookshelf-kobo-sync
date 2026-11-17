use crate::config::CONFIG;

pub fn log(msg: &str) {
  if CONFIG.debug {
    eprintln!("[bookshelf-kobo] {msg}");
  }
}
