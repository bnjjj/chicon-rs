extern crate chicon;

use std::io::prelude::*;
use std::io::SeekFrom;

use chicon::{FileSystem, OsFileSystem};

fn main() {
    let os_fs = OsFileSystem::new();

    let mut cargo_file = os_fs.open_file("Cargo.toml").unwrap();

    cargo_file.seek(SeekFrom::Start(1)).unwrap();

    let mut buffer: String = String::new();
    {
        cargo_file.read_to_string(&mut buffer).unwrap();
    }

    println!("here {:?}", buffer);

    {
        cargo_file.read_to_string(&mut buffer).unwrap();
    }
    println!("here {:?}", buffer);
}
