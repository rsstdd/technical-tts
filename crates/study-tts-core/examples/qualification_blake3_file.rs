//! Computes a BLAKE3 digest for the disposable E0-S3 qualification harness.

use std::{
    env,
    fs::File,
    io::{self, Read},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(path) = arguments.next() else {
        eprintln!("qualification BLAKE3 helper requires exactly one file");
        return ExitCode::FAILURE;
    };
    if arguments.next().is_some() {
        eprintln!("qualification BLAKE3 helper requires exactly one file");
        return ExitCode::FAILURE;
    }

    let result = File::open(path).and_then(hash_reader);
    match result {
        Ok(digest) => {
            println!("{digest}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("qualification BLAKE3 helper could not hash the governed file");
            ExitCode::FAILURE
        }
    }
}

fn hash_reader(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::hash_reader;

    #[test]
    fn t1_e0_qualification_blake3_helper_hashes_known_bytes() {
        let digest = hash_reader(Cursor::new(b"abc")).expect("hash in-memory fixture");

        assert_eq!(
            digest,
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        );
    }
}
