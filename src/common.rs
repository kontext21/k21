use log::LevelFilter;
use std::io::Write;

pub fn init_logger_exe() {
    init_logger(
        std::env::current_exe()
            .unwrap()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap(),
    );
}

pub fn init_logger(name: impl Into<String>) {
    let crate_name = name.into().replace('-', "_");

    env_logger::builder()
        .parse_default_env()
        .filter(Some(&crate_name), LevelFilter::Trace)
        .format(move |f, rec| {
            let now = humantime::format_rfc3339_millis(std::time::SystemTime::now());
            let module = rec.module_path().unwrap_or("<unknown>");
            let line = rec.line().unwrap_or(u32::MIN);
            let level = rec.level();

            writeln!(
                f,
                "[{} {} {} {}:{}] {}",
                level,
                crate_name,
                now,
                module,
                line,
                rec.args()
            )
        })
        .init();
}
