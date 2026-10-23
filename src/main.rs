use std::env;
use std::io;

fn main() -> io::Result<()> {
    cdlog::run(env::args_os())
}
