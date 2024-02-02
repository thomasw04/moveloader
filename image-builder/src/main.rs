mod byte_utils;
mod generate;
mod read;
mod verification;
mod write;

use std::io::Error;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Arguments {
    /// Write an image
    #[clap(name = "write")]
    Write(write::WriteArguments),

    /// Read an image
    #[clap(name = "read")]
    Read(read::ReadArguments),
}

fn main() -> Result<(), Error> {
    let options = Arguments::parse();

    match options {
        Arguments::Write(write_options) => write::write(write_options)?,
        Arguments::Read(read_options) => read::read(read_options)?,
    }

    Ok(())
}
