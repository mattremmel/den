use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = den::run() {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
