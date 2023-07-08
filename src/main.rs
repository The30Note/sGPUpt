use clap::{App, Arg};

fn main() {
    let app = App::new("my-rust-program")
        .version("1.0.0")
        .author("Bard")
        .about("A simple Rust program that takes in flags")
        .arg(
            Arg::new("file")
                .short("f")
                .long("file")
                .help("The file to read")
                .takes_value(true),
        )
        .arg(
            Arg::new("number")
                .short("n")
                .long("number")
                .help("A number to print")
                .takes_value(true),
        );

    let args = app.parse_args();

    let file = args.value_of("file").unwrap_or("");
    let number = args.value_of("number").unwrap_or("");

    println!("File: {}", file);
    println!("Number: {}", number);
}