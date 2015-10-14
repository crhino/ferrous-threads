extern crate ferrous_threads;
use ferrous_threads::task_runner::TaskRunner;
use std::process::Command;
use std::string::String;
use std::io::Write;
use std::io;

fn main() {
    let pool = TaskRunner::new(4);
    let n: usize = 10000;
    let mut vec = Vec::new();

    pool.enqueue(move || { sleep_and_cat() }).ok().expect("closure not queued");
    pool.enqueue(move || {
        for i in 0..n {
            vec.push(i);
            vec[i/2] = i;
        }
    }).ok().expect("closure not queued");
    pool.enqueue(move || { stdin_and_echo() }).ok().expect("closure not queued");
}

fn sleep_and_cat() {
    let mut stdout = io::stdout();
    std::thread::sleep_ms(5000);

    let output = Command::new("cat")
        .arg("README.md")
        .output()
        .unwrap_or_else(|e| { panic!("failed to execute process: {}", e) });
    assert!(stdout.write_all(String::from_utf8(output.stdout).ok().unwrap().as_bytes()).is_ok());
    assert!(stdout.write_all(String::from_utf8(output.stderr).ok().unwrap().as_bytes()).is_ok());
}

fn stdin_and_echo() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut buf = String::new();
    let echo = String::from("You said: ");
    let magic = String::from("Rhino\n");
    assert!(stdout.write_all(magic.as_bytes()).is_ok());

    while buf != magic {
        buf.clear();
        assert!(stdin.read_line(&mut buf).is_ok());
        assert!(stdout.write_all(echo.as_bytes()).is_ok());
        assert!(stdout.write_all(buf.as_bytes()).is_ok());
    }

    assert!(stdout.write_all(String::from("Goodbye!\n").as_bytes()).is_ok());
}
