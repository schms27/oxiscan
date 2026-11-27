//! "Open" commands: launch a browser, terminal, file shares or remote desktop for a host.

use std::net::Ipv4Addr;

pub enum Opener {
    Url(String),
    Terminal(String),
    Shares(Ipv4Addr),
    Rdp(Ipv4Addr),
}

pub fn ping_cmd(ip: Ipv4Addr) -> String {
    if cfg!(windows) {
        format!("ping -t {}", ip)
    } else {
        format!("ping {}", ip)
    }
}

pub fn trace_cmd(ip: Ipv4Addr) -> String {
    if cfg!(windows) {
        format!("tracert {}", ip)
    } else {
        format!("traceroute {}", ip)
    }
}

impl Opener {
    pub fn run(&self) -> std::io::Result<()> {
        use std::process::Command;
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            match self {
                Opener::Url(u) => Command::new("cmd")
                    .args(["/C", "start", "", u])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                    .map(|_| ()),
                Opener::Terminal(c) => Command::new("cmd")
                    .args(["/K", c])
                    .creation_flags(CREATE_NEW_CONSOLE)
                    .spawn()
                    .map(|_| ()),
                Opener::Shares(ip) => Command::new("explorer")
                    .arg(format!("\\\\{}", ip))
                    .spawn()
                    .map(|_| ()),
                Opener::Rdp(ip) => Command::new("mstsc").arg(format!("/v:{}", ip)).spawn().map(|_| ()),
            }
        }
        #[cfg(not(windows))]
        {
            let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
            match self {
                Opener::Url(u) => Command::new(opener).arg(u).spawn().map(|_| ()),
                Opener::Terminal(c) => {
                    if cfg!(target_os = "macos") {
                        Command::new("osascript")
                            .args(["-e", &format!("tell app \"Terminal\" to do script \"{}\"", c)])
                            .spawn()
                            .map(|_| ())
                    } else {
                        Command::new("x-terminal-emulator").args(["-e", c]).spawn().map(|_| ())
                    }
                }
                Opener::Shares(ip) => Command::new(opener).arg(format!("smb://{}/", ip)).spawn().map(|_| ()),
                Opener::Rdp(ip) => Command::new(opener).arg(format!("rdp://{}", ip)).spawn().map(|_| ()),
            }
        }
    }
}
