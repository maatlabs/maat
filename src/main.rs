fn main() -> std::io::Result<()> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| String::from("user"));

    println!("\nHello {username}! This is the Maat programming language!");
    println!("Feel free to type in commands\n");

    let reader = std::io::stdin().lock();
    let mut writer = std::io::stdout().lock();

    maat::interpreter::repl::start(reader, &mut writer)
}
