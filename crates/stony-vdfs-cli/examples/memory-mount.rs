//! Minimal example: create a memory backend, write a file, list and read it back.

use stony_vdfs_backend_memory::MemoryBackend;
use stony_vdfs_core::{OpenFlags, VfsBackend, VfsPath};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = MemoryBackend::new();

    let greeting = VfsPath::new("/hello.txt").ok_or("invalid path")?;
    let mut handle = backend
        .open(&greeting, OpenFlags::CREATE | OpenFlags::WRITE)
        .await?;
    handle.write(0, b"hello, world\n").await?;
    handle.flush().await?;

    let root = VfsPath::root();
    println!("Contents of {}:", root);
    for entry in backend.list(&root).await? {
        let meta = backend.metadata(&entry).await?;
        println!("  {entry}  ({} bytes, {})", meta.size, meta.kind.as_str());
    }

    let mut reader = backend.open(&greeting, OpenFlags::READ).await?;
    let data = reader.read(0, 4096).await?;
    print!("{}", String::from_utf8_lossy(&data));

    Ok(())
}
