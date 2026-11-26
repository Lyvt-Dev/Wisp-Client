#![cfg_attr(
    all(target_os = "windows", feature = "gui"),
    windows_subsystem = "windows"
)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wisp_launcher::{
    auth, engine,
    i18n::{tr, trf},
};
use wisp_launcher::{launcher_home, Profile, Result, Settings};

#[cfg(feature = "gui")]
mod account_art;
#[cfg(feature = "beta")]
mod beta;
#[cfg(feature = "gui")]
mod dashboard;

#[cfg(feature = "gui")]
mod art;
#[cfg(all(feature = "gui", test))]
mod glass;
#[cfg(all(feature = "gui", test))]
mod scene;
#[cfg(all(feature = "gui", test))]
#[allow(dead_code)]
mod sky;

#[derive(Parser)]
#[command(version, about = "Wisp launcher for the Rust Minecraft engine")]
struct Cli {
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Subcommand)]
enum Action {
    /// Register an existing local build. Paths are stored as absolute paths.
    Add {
        name: String,
        #[arg(long)]
        binary: PathBuf,
        #[arg(long)]
        resources: PathBuf,
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        online: bool,
        /// Extra game arguments, passed literally without a shell.
        #[arg(last = true)]
        args: Vec<String>,
    },
    List,
    Select {
        name: String,
    },
    /// Forget a profile; keeps the game, worlds and resources on disk.
    Remove {
        name: String,
    },
    /// Launch the selected profile, or the named profile; wait for its exit code.
    Play {
        name: Option<String>,
    },
    /// Prepare resources from the player's own Minecraft installation.
    Setup {
        #[arg(long)]
        minecraft_dir: PathBuf,
        #[arg(long)]
        server_jar: PathBuf,
    },
    /// Download and prepare Minecraft for the bundled Rust engine. Requires a signed-in owner.
    Install {
        /// Start the Rust game after installation.
        #[arg(long)]
        play: bool,
    },
    /// Microsoft account operations. Output never includes credentials.
    Account {
        #[command(subcommand)]
        command: AccountAction,
    },
    /// The Microsoft sign-in window the launcher opens for itself (`auth::window`).
    #[command(hide = true)]
    SignInWindow {
        url: String,
        redirect: String,
        data: PathBuf,
    },
}

#[derive(Subcommand)]
enum AccountAction {
    Status,
    SignIn,
    SignOut,
    Check,
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    if let Some(Action::SignInWindow {
        url,
        redirect,
        data,
    }) = &cli.command
    {
        #[cfg(all(windows, feature = "gui"))]
        auth::window::run(url, redirect, data.clone());
        #[cfg(not(all(windows, feature = "gui")))]
        {
            let _ = (url, redirect, data);
            return Err("The sign-in window needs the Windows launcher".into());
        }
    }
    let home = launcher_home()?;
    let mut settings = Settings::load(&home)?;
    let Some(action) = cli.command else {
        #[cfg(feature = "gui")]
        {
            dashboard::run(home, settings);
            return Ok(0);
        }
        #[cfg(not(feature = "gui"))]
        return Err("Use a subcommand, or build with the gui feature".into());
    };
    match action {
        Action::Add {
            name,
            binary,
            resources,
            home: game_home,
            online,
            args,
        } => {
            let mut profile = Profile::new(name, &binary, &resources, &game_home)?;
            profile.args = args;
            profile.use_account = online;
            settings.add(profile)?;
            settings.save(&home)?;
        }
        Action::List => {
            for p in &settings.profiles {
                println!(
                    "{} {}\n  {}\n  {}",
                    if settings.selected.as_ref() == Some(&p.name) {
                        "*"
                    } else {
                        " "
                    },
                    p.name,
                    p.binary.display(),
                    p.game_home.display()
                );
            }
        }
        Action::Select { name } => {
            settings.find(&name).ok_or("Unknown profile")?;
            settings.selected = Some(name);
            settings.save(&home)?;
        }
        Action::Remove { name } => {
            settings.find(&name).ok_or("Unknown profile")?;
            settings.profiles.retain(|p| p.name != name);
            if settings.selected.as_ref() == Some(&name) {
                settings.selected = settings.profiles.first().map(|p| p.name.clone());
            }
            settings.save(&home)?;
        }
        Action::Play { name } => {
            let profile = match name.as_deref() {
                Some(name) => settings.find(name),
                None => settings.current(),
            }
            .ok_or("No profile selected; add a local build first")?;
            let account = if profile.use_account {
                Some(auth::for_game(&home, None, &|phase| {
                    eprintln!("{}", phase.label())
                })?)
            } else {
                None
            };
            let credentials = account.as_ref().map(auth::SignedIn::credentials);
            return Ok(profile
                .launch_with_credentials(credentials.as_ref())?
                .wait()?
                .code()
                .unwrap_or(1));
        }
        Action::Install { play } => {
            let names = settings
                .profiles
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>();
            let installed = wisp_launcher::installation::install(
                &home,
                None,
                &names,
                &std::sync::atomic::AtomicBool::new(false),
                &|step| eprintln!("{}", step.message),
            )?;
            let profile = installed.profile;
            settings.add(profile.clone())?;
            settings.save(&home)?;
            if play {
                return Ok(profile
                    .launch_with_credentials(Some(&installed.account.credentials()))?
                    .wait()?
                    .code()
                    .unwrap_or(1));
            }
        }
        Action::Setup {
            minecraft_dir,
            server_jar,
        } => {
            let resources = engine::prepare(
                &home,
                &minecraft_dir,
                &server_jar,
                &std::sync::atomic::AtomicBool::new(false),
                &|step| eprintln!("{}", step.message),
            )?;
            let names = settings
                .profiles
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>();
            settings.add(engine::new_profile(&home, &resources, &names)?)?;
            settings.save(&home)?;
        }
        // Handled before the launcher's home is read.
        Action::SignInWindow { .. } => {}
        Action::Account { command } => match command {
            AccountAction::Status => match auth::saved(&home)? {
                Some(account) => println!("{} {}", account.account.name, account.account.uuid),
                None => println!("{}", tr("Not signed in.")),
            },
            AccountAction::SignIn => {
                let client = auth::client_id().ok_or(auth::NOT_CONFIGURED)?;
                let ep = auth::Endpoints::live();
                let code = auth::device_code(&ep, &client)?;
                println!("https://www.microsoft.com/link\n{}", code.user_code);
                let ms = auth::wait_for_sign_in(&ep, &client, &code, &|| false)?;
                let (account, secrets) = auth::finish_sign_in(&ep, ms)?;
                let signed = auth::SignedIn::from_tokens(account, secrets);
                auth::save(&home, &signed)?;
                println!(
                    "{}",
                    trf("Signed in as {name}.", &[("name", &signed.account.name)])
                );
            }
            AccountAction::SignOut => {
                auth::sign_out(&home)?;
                println!("{}", tr("Signed out."));
            }
            AccountAction::Check => {
                let account = auth::for_game(&home, None, &|phase| eprintln!("{}", phase.label()))?;
                println!(
                    "{}",
                    trf("Account ready: {name}", &[("name", &account.account.name)])
                );
            }
        },
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
