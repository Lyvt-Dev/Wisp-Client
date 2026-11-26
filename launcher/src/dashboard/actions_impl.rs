use super::*;

impl Launcher {
    pub(super) fn play(&mut self, cx: &mut Context<Self>) {
        self.launch_with(Vec::new(), cx);
    }

    /// Plays, opening a world (`--quickPlaySingleplayer`) or server straight away.
    pub(super) fn launch_with(&mut self, extra: Vec<String>, cx: &mut Context<Self>) {
        if self.launching
            || self.child.is_some()
            || self.picking
            || self.auth_busy
            || self.setup_busy
        {
            return;
        }
        let Some(mut profile) = self.settings.current().cloned() else {
            self.install(cx);
            return;
        };
        profile.args.extend(self.prefs.game_args());
        self.last_server = extra
            .iter()
            .position(|a| a == "--quickPlayMultiplayer")
            .and_then(|i| extra.get(i + 1).cloned());
        profile.args.extend(extra);
        self.write_cosmetic_prefs();
        if profile.use_account && self.account.is_none() {
            self.page = Page::Account;
            self.status = tr(auth::NEEDS_OWNER);
            cx.notify();
            return;
        }
        self.launching = true;
        self.failed = false;
        self.status = tr("Starting Minecraft…");
        self.auth_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.auth_cancel.clone();
        let home = self.home.clone();
        let wallet = self.wallet.clone();
        let cached = self.account.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let account = if profile.use_account {
                    Some(auth::for_game(&home, cached, &|phase| {
                        let _ = sender.send(Event::Status(phase.label()));
                    })?)
                } else {
                    None
                };
                if cancel.load(Ordering::Relaxed) {
                    return Err(tr("Launch cancelled."));
                }
                let credentials = account.as_ref().map(auth::SignedIn::credentials);
                // What the Wisp server says is worn.
                let worn = wallet
                    .dress(&wisp_launcher::shop::worn_dir(&home))
                    .ok()
                    .filter(|n| *n > 0)
                    .map(|_| wisp_launcher::shop::worn_dir(&home));
                let child = profile
                    .launch_dressed(credentials.as_ref(), worn.as_deref())
                    .map_err(|e| e.to_string())?;
                Ok((child, account, profile.name))
            })();
            let _ = sender.send(Event::Launched(result));
        });
        cx.notify();
    }

    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(profile) = self.settings.profiles.get(index) else {
            return;
        };
        let mut settings = self.settings.clone();
        settings.selected = Some(profile.name.clone());
        match settings.save(&self.home) {
            Ok(()) => {
                self.settings = settings;
                self.failed = false;
                self.status = tr("Instance selected. Ready to play.");
            }
            Err(error) => self.error(error.to_string()),
        }
        cx.notify();
    }

    pub(super) fn next(&mut self, cx: &mut Context<Self>) {
        if self.settings.profiles.is_empty() {
            return;
        }
        let index = self
            .settings
            .profiles
            .iter()
            .position(|p| Some(&p.name) == self.settings.selected.as_ref())
            .unwrap_or(0);
        self.select((index + 1) % self.settings.profiles.len(), cx);
    }

    pub(super) fn new_instance(&mut self, cx: &mut Context<Self>) {
        self.install(cx);
    }

    pub(super) fn install(&mut self, cx: &mut Context<Self>) {
        if self.picking
            || self.setup_busy
            || self.auth_busy
            || self.launching
            || self.child.is_some()
        {
            return;
        }
        if self.account.is_none() {
            self.install_after_sign_in = true;
            self.page = Page::Account;
            self.status = tr("Sign in with the Microsoft account that owns Minecraft. Game installation will continue automatically.");
            self.failed = false;
            cx.notify();
            return;
        }
        self.page = Page::Setup;
        self.setup_busy = true;
        self.failed = false;
        self.setup_fraction = None;
        self.status = tr("Checking your account and installing Minecraft for the Rust client…");
        self.setup_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.setup_cancel.clone();
        let home = self.home.clone();
        let account = self.account.clone();
        let names = self
            .settings
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let installed = installation::install(&home, account, &names, &cancel, &|step| {
                let _ = sender.send(Event::SetupProgress(step));
            });
            let _ = sender.send(Event::Installed(installed));
        });
        cx.notify();
    }

    pub(super) fn use_prepared(&mut self, cx: &mut Context<Self>) {
        if self.picking || self.setup_busy {
            return;
        }
        if engine::bundled().is_none() {
            self.page = Page::Setup;
            self.error(tr(
                "Install the full Wisp setup to use the bundled Rust engine.",
            ));
            cx.notify();
            return;
        }
        self.picking = true;
        self.status =
            tr("Choose a prepared Minecraft resources folder, or cancel and use Game setup.");
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Choose prepared resources").into()),
        });
        let sender = self.sender.clone();
        let home = self.home.clone();
        let names = self
            .settings
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let online = self.account.is_some();
        cx.spawn(async move |this, cx| {
            let result: wisp_launcher::Result<Option<PathBuf>> =
                async { Ok(prompt.await??.and_then(|mut paths| paths.pop())) }.await;
            match result {
                Ok(Some(resources)) => {
                    std::thread::spawn(move || {
                        let result = engine::new_profile(&home, &resources, &names).map(|mut p| {
                            p.use_account = online;
                            p
                        });
                        let _ = sender.send(Event::Added(result));
                    });
                }
                other => {
                    let _ = this.update(cx, |this, cx| {
                        this.picking = false;
                        match other {
                            Err(error) => this.error(error.to_string()),
                            _ => this.status = tr("Instance creation cancelled."),
                        };
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub(super) fn import(&mut self, cx: &mut Context<Self>) {
        if self.picking || self.setup_busy {
            return;
        }
        self.picking = true;
        self.status = tr("Choose an existing Rust game executable.");
        let binary = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose Rust game executable").into()),
        });
        cx.spawn(async move |this, cx| {
            let result: wisp_launcher::Result<Option<Profile>> = async {
                let Some(binary) = binary.await??.and_then(|mut p| p.pop()) else {
                    return Ok(None);
                };
                let resources = cx.update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(tr("Choose prepared resources").into()),
                    })
                });
                let Some(resources) = resources.await??.and_then(|mut p| p.pop()) else {
                    return Ok(None);
                };
                let game = cx.update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(tr("Choose game folder").into()),
                    })
                });
                let Some(game) = game.await??.and_then(|mut p| p.pop()) else {
                    return Ok(None);
                };
                let name = game
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Imported instance")
                    .to_owned();
                Ok(Some(Profile::new(name, &binary, &resources, &game)?))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.picking = false;
                match result {
                    Ok(Some(profile)) => {
                        this.save_profile(profile);
                    }
                    Ok(None) => {
                        this.status = tr("Import cancelled. Your existing files are unchanged.")
                    }
                    Err(error) => this.error(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn prepare(&mut self, cx: &mut Context<Self>) {
        if self.picking || self.setup_busy {
            return;
        }
        if engine::bundled().is_none() {
            self.error(tr("Install the full Wisp setup first."));
            cx.notify();
            return;
        }
        self.picking = true;
        let minecraft = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Choose your Minecraft installation folder").into()),
        });
        let home = self.home.clone();
        let sender = self.sender.clone();
        let names = self
            .settings
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let online = self.account.is_some();
        self.setup_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.setup_cancel.clone();
        cx.spawn(async move |this, cx| {
            let result: wisp_launcher::Result<Option<(PathBuf, PathBuf)>> = async {
                let Some(minecraft) = minecraft.await??.and_then(|mut p| p.pop()) else {
                    return Ok(None);
                };
                let server = cx.update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some(tr("Choose the Minecraft 26.3 server JAR").into()),
                    })
                });
                Ok(server
                    .await??
                    .and_then(|mut p| p.pop())
                    .map(|server| (minecraft, server)))
            }
            .await;
            match result {
                Ok(Some((minecraft, server))) => {
                    let _ = this.update(cx, |this, cx| {
                        this.picking = false;
                        this.setup_busy = true;
                        this.setup_fraction = None;
                        this.failed = false;
                        this.status = tr("Preparing your Minecraft resources…");
                        cx.notify();
                    });
                    std::thread::spawn(move || {
                        let result =
                            engine::prepare(&home, &minecraft, &server, &cancel, &|progress| {
                                let _ = sender.send(Event::SetupProgress(progress));
                            })
                            .and_then(|resources| engine::new_profile(&home, &resources, &names))
                            .map(|mut p| {
                                p.use_account = online;
                                p
                            });
                        let _ = sender.send(Event::Prepared(result));
                    });
                }
                other => {
                    let _ = this.update(cx, |this, cx| {
                        this.picking = false;
                        match other {
                            Err(error) => this.error(error.to_string()),
                            _ => this.status = tr("Game setup cancelled."),
                        };
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Adds an account in the sign-in window: Microsoft's own sign-in page opens in a launcher
    /// window, and the account becomes the one that plays.
    pub(super) fn sign_in(&mut self, cx: &mut Context<Self>) {
        if self.auth_busy || self.launching || self.setup_busy {
            return;
        }
        self.page = Page::Account;
        let Some(client) = auth::client_id() else {
            self.error(tr(auth::NOT_CONFIGURED));
            cx.notify();
            return;
        };
        self.auth_busy = true;
        self.browser_waiting = true;
        self.failed = false;
        self.device = None;
        self.status = tr("Finish signing in in the Microsoft window…");
        self.auth_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.auth_cancel.clone();
        let sender = self.sender.clone();
        let home = self.home.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let ep = auth::Endpoints::live();
                let data = home.join("sign-in-window");
                let ms = auth::browser_sign_in(&ep, &client, &|url| {
                    auth::sign_in_window(url, &data, &|| cancel.load(Ordering::Relaxed))
                })?;
                let (account, secrets) = auth::finish_sign_in_with_progress(&ep, ms, &|phase| {
                    let _ = sender.send(Event::Status(phase.label()));
                })?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(tr("Sign-in cancelled."));
                }
                let account = auth::SignedIn::from_tokens(account, secrets);
                let _ = sender.send(Event::Status(auth::Phase::Saving.label()));
                auth::save(&home, &account)?;
                Ok(account)
            })();
            let _ = sender.send(Event::SignedIn(result));
        });
        cx.notify();
    }

    /// Makes the kept account `uuid` the one that plays.
    pub(super) fn switch_account(&mut self, uuid: String, cx: &mut Context<Self>) {
        if self.auth_busy || self.launching || self.setup_busy {
            return;
        }
        match auth::switch(&self.home, &uuid) {
            Ok(account) => {
                self.status = trf("Playing as {name}.", &[("name", &account.account.name)]);
                self.failed = false;
                self.account = Some(account);
                self.load_avatar();
                self.refresh_accounts();
            }
            Err(e) => self.error(e),
        }
        cx.notify();
    }

    /// Forgets the kept account `uuid`.
    pub(super) fn remove_account(&mut self, uuid: String, cx: &mut Context<Self>) {
        if self.auth_busy || self.launching || self.setup_busy {
            return;
        }
        match auth::remove(&self.home, &uuid) {
            Ok(next) => {
                self.failed = false;
                self.status = tr("Account removed from Wisp.");
                let changed = self.account.as_ref().map(|a| a.account.uuid.clone())
                    != next.as_ref().map(|a| a.account.uuid.clone());
                self.account = next;
                if changed {
                    self.avatar_key = None;
                    self.avatar = None;
                    self.load_avatar();
                }
                self.refresh_accounts();
            }
            Err(e) => self.error(e),
        }
        cx.notify();
    }

    /// Reads the kept accounts again (names and skins only).
    pub(super) fn refresh_accounts(&mut self) {
        if let Ok((list, _)) = auth::accounts(&self.home) {
            self.accounts = list;
        }
        // Friends belong to the account that plays: sign in to the Wisp server again.
        self.friends = None;
        self.friends_state = None;
        self.friends_notice = None;
        self.friends_send(super::friends_view::FriendCmd::Reset);
    }

    /// Adds an account with a code typed at microsoft.com/link (for when the browser can't).
    pub(super) fn sign_in_with_code(&mut self, cx: &mut Context<Self>) {
        if self.auth_busy || self.launching || self.setup_busy {
            return;
        }
        self.page = Page::Account;
        let Some(client) = auth::client_id() else {
            self.error(tr(auth::NOT_CONFIGURED));
            cx.notify();
            return;
        };
        self.auth_busy = true;
        self.failed = false;
        self.device = None;
        self.status = tr("Getting a Microsoft sign-in code…");
        self.auth_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.auth_cancel.clone();
        let sender = self.sender.clone();
        let home = self.home.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let ep = auth::Endpoints::live();
                let code = auth::device_code(&ep, &client)?;
                let _ = sender.send(Event::Code(code.clone()));
                let ms = auth::wait_for_sign_in(&ep, &client, &code, &|| {
                    cancel.load(Ordering::Relaxed)
                })?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(tr("Sign-in cancelled."));
                }
                let (account, secrets) = auth::finish_sign_in_with_progress(&ep, ms, &|phase| {
                    let _ = sender.send(Event::Status(phase.label()));
                })?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(tr("Sign-in cancelled."));
                }
                let account = auth::SignedIn::from_tokens(account, secrets);
                let _ = sender.send(Event::Status(auth::Phase::Saving.label()));
                auth::save(&home, &account)?;
                Ok(account)
            })();
            let _ = sender.send(Event::SignedIn(result));
        });
        cx.notify();
    }

    pub(super) fn toggle_account(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.settings.current() else {
            self.page = Page::Instances;
            cx.notify();
            return;
        };
        if !current.use_account && self.account.is_none() {
            self.page = Page::Account;
            cx.notify();
            return;
        }
        let name = current.name.clone();
        let mut settings = self.settings.clone();
        if let Some(profile) = settings.profiles.iter_mut().find(|p| p.name == name) {
            profile.use_account = !profile.use_account;
        }
        match settings.save(&self.home) {
            Ok(()) => {
                self.settings = settings;
                self.failed = false;
                self.status = tr("Account mode updated for this instance.");
            }
            Err(error) => self.error(error.to_string()),
        }
        cx.notify();
    }

    pub(super) fn show_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.exists() {
            cx.reveal_path(&path);
        } else {
            self.status =
                tr("This path does not exist yet. Start the game or complete setup first.");
            cx.notify();
        }
    }

    pub(super) fn open_log(&mut self, cx: &mut Context<Self>) {
        let path = self
            .settings
            .current()
            .map(|p| p.game_home.join("logs/local-launcher.log"))
            .unwrap_or_else(|| self.home.join("logs/prepare.log"));
        self.show_path(path, cx);
    }

    pub(super) fn open_game_folder(&mut self, folder: &'static str, cx: &mut Context<Self>) {
        let Some(profile) = self.settings.current() else {
            self.page = Page::Instances;
            cx.notify();
            return;
        };
        let path = profile.game_home.join(folder);
        match std::fs::create_dir_all(&path) {
            Ok(()) => cx.reveal_path(&path),
            Err(error) => {
                self.error(error.to_string());
                cx.notify();
            }
        }
    }
}
