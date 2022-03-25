use colored::Colorize;

pub fn info(target: impl AsRef<str>, msg: impl AsRef<str>) {
    let target = format!("[{}]", target.as_ref());

    println!("{} {}", target.blue(), msg.as_ref());
}

pub fn error(target: impl AsRef<str>, msg: impl AsRef<str>) {
    let target = format!("[{}]", target.as_ref());

    eprintln!("{} {}", target.blue(), msg.as_ref().red());
}
