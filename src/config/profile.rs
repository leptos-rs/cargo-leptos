use core::fmt;

#[derive(Debug)]
pub enum Profile {
    Debug,
    Release,
    Named(String),
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
            Self::Named(name) => write!(f, "{}", name),
        }
    }
}

impl Profile {
    pub fn new(is_release: bool, release: &Option<String>, debug: &Option<String>) -> Self {
        if is_release {
            if let Some(release) = release {
                Self::Named(release.clone())
            } else {
                Self::Release
            }
        } else if let Some(debug) = debug {
            Self::Named(debug.clone())
        } else {
            Self::Debug
        }
    }

    pub fn add_to_args(&self, args: &mut Vec<String>) {
        match self {
            Self::Debug => {}
            Self::Release => {
                args.push("--release".to_string());
            }
            Self::Named(name) => {
                args.push(format!("--profile={}", name));
            }
        }
    }
}
