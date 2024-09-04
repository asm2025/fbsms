#[cfg(debug_assertions)]
pub fn is_debug() -> bool {
    true
}

#[cfg(not(debug_assertions))]
pub fn is_debug() -> bool {
    false
}

#[cfg(debug_assertions)]
pub fn num_threads() -> usize {
    1
}

#[cfg(not(debug_assertions))]
pub fn num_threads() -> usize {
    num_cpus::get()
}
