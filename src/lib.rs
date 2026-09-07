#[cfg(not(target_pointer_width = "64"))]
compile_error!("Grossular currently requires a 64-bit target");

mod address;
mod record;
