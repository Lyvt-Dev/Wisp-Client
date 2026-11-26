# Microsoft sign-in for Wisp

Wisp uses the Microsoft device-code flow. Click **Sign in with Microsoft**,
open **microsoft.com/link**, and enter the code displayed by Wisp. Your password
is entered on Microsoft's website, never in the launcher.

The Rust engine receives the verified Minecraft account through its standard-input
session pipe. Access tokens do not appear in game arguments, launcher logs, or
profile settings. On Windows, saved sign-ins are encrypted by Windows DPAPI for the
current user. Signing out removes Wisp's saved sign-in and preserves game files.
Other platforms currently retain the sign-in in memory for that launcher session.

## Wisp registration

Version 0.4.1 and newer include Wisp's owner-provided public client ID:
7a968ec3-4976-49b6-980f-4dcb76b3a95e. Microsoft's personal-account endpoint accepted
the device-code request and returned authorization_pending when polled. These
checks verify device authorization setup, not a completed user/Xbox/Minecraft login.

Open **Account > Sign in with Microsoft** in the installed launcher to get your
own current code, then visit microsoft.com/link. Codes expire; old test codes
are not included in the installer. No manual client-ID configuration is needed.

## Registering a different application

The link page alone cannot issue an Wisp sign-in code. Microsoft requires the
application's public client ID when requesting a device code. Wisp uses its own
registration and does not generate a fake code or borrow another launcher's ID.

1. In Microsoft Entra **App registrations**, register **Wisp** for personal
   Microsoft accounts.
2. Enable **Allow public client flows**. The device-code flow needs no client
   secret and no local redirect server.
3. Request Minecraft API approval for the client ID using the official
   [Minecraft App ID review](https://aka.ms/mce-reviewappid).
4. Set the public client ID before building:

       $env:WISP_MSA_CLIENT_ID = '<Wisp application client ID>'
       ./packaging/windows/build-installer.ps1

For GitHub Actions, the optional repository variable WISP_MSA_CLIENT_ID overrides
the default; an unset or empty build variable keeps Wisp's registration.
The ID is public application configuration, not a secret. A runtime variable with
the same name overrides the built-in value; explicitly empty disables sign-in.

No client secret is used or should be supplied.

The requested scopes are XboxLive.signin offline_access. Wisp exchanges the
Microsoft credential through Xbox Live and XSTS, obtains a Minecraft session,
checks ownership, and loads the Minecraft profile. New app registrations can be
refused by Minecraft Services until approved. Refresh runs before account-based
game launches, and a rejected refresh asks the player to sign in again.

Existing local instances stay in local-player mode. Choose **Switch account mode**
for an instance to use the selected Microsoft account. Minecraft gameplay remains
the Rust engine; Wisp does not start the Java client.

Protocol reference:
[Microsoft device authorization flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code).
The Xbox/Minecraft exchange and Rust session contract are adapted from Wisp at
2181ab22ebc888b62cf3303b9aaa6c215cce6392, under the repository's MIT license.

## Automatic game setup

From 0.4.2, Install & play remembers the setup request while you sign in. After
ownership is verified, Wisp downloads the required Minecraft files, server JAR
and preparation runtime from Mojang, prepares resources, creates an instance and
starts the bundled Rust game. No manual folder/JDK setup is needed.
