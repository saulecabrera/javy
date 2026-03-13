use waffle::{Local, entity::EntityRef};

/// The `JSContext` is always local 0.
#[inline]
pub fn context() -> Local {
    Local::new(0)
}

/// Get the local at index, optionally allowing the caller to transform the
/// index.
pub fn at<F>(index: usize, map_index: F) -> Local
where
    F: Fn(usize) -> usize,
{
    Local::new(map_index(index))
}
