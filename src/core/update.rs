use std::time::Duration;

use semver::Version;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Update checking — "is there a newer Drift than the one running?"
//
// Deliberately in `core`, and deliberately free of GPUI: everything here takes
// plain values and returns plain values, so the whole decision — which
// releases count, which one wins, whether it beats the running build — is
// unit-testable against fixture JSON with no window, no runtime, and no
// network. `ui::app_view` is the only caller; it runs [`check`] on the
// background executor and renders whatever comes back.
//
// The source of truth is the GitHub Releases API of the repository this binary
// was built from. Nothing is downloaded or installed: the check ends at a URL
// the user can open. An updater that replaces the running app would need code
// signing and a much larger trust story than a dashboard that reads local
// files deserves.
//
// Two channels, and the rule between them is the whole design:
//
//   * `Stable` sees releases that are neither marked pre-release on GitHub nor
//     carry a semver pre-release suffix.
//   * `Beta` sees everything, betas included — a beta tester is someone who
//     accepts the newest build, whichever kind it is. Semver ordering already
//     says `0.2.0` > `0.2.0-beta.2`, so a beta user is moved onto the stable
//     build the moment it ships rather than being stranded on the beta line.
// ---------------------------------------------------------------------------

/// The repository releases are published to.
///
/// Read from the environment at build time so a fork — or a rename of the
/// repository — needs a build flag rather than a patch: CI passes
/// `DRIFT_UPDATE_REPO=${{ github.repository }}`.
pub const REPO: &str = match option_env!("DRIFT_UPDATE_REPO") {
    // An empty value is a build script that meant to set it and did not; the
    // default is a better answer than a request to `/repos//releases`.
    Some(repo) if !repo.is_empty() => repo,
    _ => "duongductrong/drift",
};

/// How many releases to ask GitHub for. Enough that a run of betas cannot
/// push the newest stable release off the end of the page.
const RELEASES_PER_PAGE: usize = 30;

/// Applies to the whole request, connect through body. A check that has not
/// finished by then is not worth blocking a settings dialog on.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// Which releases a user has opted into seeing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Stable, Channel::Beta];

    pub fn label(&self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Beta => "Beta",
        }
    }

    /// The stable name written to the settings file and to release metadata.
    pub fn key(&self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }

    pub fn from_key(key: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.key() == key)
    }

    /// Whether a release on this channel is offered to `self`'s subscribers.
    fn accepts(&self, release: Channel) -> bool {
        match self {
            // Betas are opt-in; a stable user is never shown one.
            Channel::Stable => release == Channel::Stable,
            // A beta user takes the newest build of either kind.
            Channel::Beta => true,
        }
    }

    /// The channel a *build* belongs to, inferred from its own version.
    ///
    /// Used as the default subscription: someone running `0.2.0-beta.1` asked
    /// for betas by installing one, and should keep getting them.
    pub fn of_version(version: &Version) -> Channel {
        if version.pre.is_empty() {
            Channel::Stable
        } else {
            Channel::Beta
        }
    }
}

// ---------------------------------------------------------------------------
// The running build
// ---------------------------------------------------------------------------

/// The version of the running binary, from `Cargo.toml`.
///
/// Release builds stamp the version into `Cargo.toml` before compiling, so a
/// beta binary reports e.g. `0.2.0-beta.1` and compares correctly against
/// what GitHub lists.
pub fn current_version() -> Version {
    // The literal comes from Cargo, which rejects a non-semver version, so a
    // parse failure here is impossible in a build that exists at all.
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version::new(0, 0, 0))
}

/// The channel this build was published on — see [`Channel::of_version`].
pub fn current_channel() -> Channel {
    Channel::of_version(&current_version())
}

// ---------------------------------------------------------------------------
// Releases
// ---------------------------------------------------------------------------

/// One published release, reduced to what the app actually shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub channel: Channel,
    /// The release title, falling back to the tag when GitHub has none.
    pub name: String,
    /// The generated release notes, as Markdown.
    pub notes: String,
    /// The release page — what "Download" opens.
    pub url: String,
    /// The DMG built for this machine's architecture, when the release has one.
    pub dmg_url: Option<String>,
}

impl Release {
    /// `0.2.0-beta.1 (Beta)` — what the dialog puts in front of the user.
    pub fn display_version(&self) -> String {
        format!("{} ({})", self.version, self.channel.label())
    }
}

/// The answer to "should this user be told about something?".
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    UpToDate,
    Available(Release),
}

/// Why a check could not produce an answer.
///
/// Every variant is a *report*, not a failure to handle: the caller renders
/// the message and the app carries on. Being offline is the normal case, not
/// an exceptional one.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateError {
    /// The request never completed — offline, DNS, TLS, timeout.
    Network(String),
    /// GitHub answered, but not with a release list (rate limit, 404, 5xx).
    Server(u16),
    /// The response was not the JSON we expect.
    Malformed(String),
    /// The repository has no release this channel can offer.
    NoReleases,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(_) => write!(f, "Could not reach GitHub"),
            UpdateError::Server(403 | 429) => write!(f, "GitHub rate limit reached"),
            UpdateError::Server(404) => write!(f, "No releases published yet"),
            UpdateError::Server(status) => write!(f, "GitHub returned {status}"),
            UpdateError::Malformed(_) => write!(f, "Unexpected response from GitHub"),
            UpdateError::NoReleases => write!(f, "No releases published yet"),
        }
    }
}

// ---------------------------------------------------------------------------
// Check state
//
// The check is asynchronous and the user can ask for it again while one is in
// flight, so "what should the dialog say right now" is a small state machine.
// It lives here rather than in the view because none of it is about drawing:
// the view holds one of these, hands it to the dialog, and renders the line
// [`CheckState::summary`] returns.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub enum CheckState {
    /// Nothing asked for yet — the launch check is off, or has not run.
    #[default]
    Idle,
    Checking,
    Done(Status),
    /// A check that could not answer. Kept rather than reset to `Idle` so the
    /// user learns the check failed instead of watching it silently do nothing.
    Failed(UpdateError),
}

impl CheckState {
    /// The one line the settings dialog shows under "Updates".
    pub fn summary(&self, current: &Version) -> String {
        match self {
            CheckState::Idle => format!("Version {current}"),
            CheckState::Checking => "Checking for updates…".to_owned(),
            CheckState::Done(Status::UpToDate) => format!("Up to date — version {current}"),
            CheckState::Done(Status::Available(release)) => {
                format!("Version {} is available", release.display_version())
            }
            CheckState::Failed(error) => error.to_string(),
        }
    }

    /// The release to offer, when there is one. Drives both the download
    /// button and the toolbar's badge.
    pub fn available(&self) -> Option<&Release> {
        match self {
            CheckState::Done(Status::Available(release)) => Some(release),
            _ => None,
        }
    }

    pub fn is_checking(&self) -> bool {
        matches!(self, CheckState::Checking)
    }
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

/// Ask GitHub what the newest release on `channel` is and compare it with the
/// running build. Blocking — call it on a background thread.
pub fn check(channel: Channel) -> Result<Status, UpdateError> {
    let body = fetch_releases(REPO)?;
    let releases = parse_releases(&body)?;
    evaluate(&releases, &current_version(), channel)
}

/// The pure half of [`check`]: given the releases that exist, decide.
///
/// Split out so the interesting rules — channel filtering, semver ordering,
/// pre-release handling — are tested without a network.
pub fn evaluate(
    releases: &[Release],
    current: &Version,
    channel: Channel,
) -> Result<Status, UpdateError> {
    let Some(latest) = latest_for(releases, channel) else {
        return Err(UpdateError::NoReleases);
    };

    // `Version`'s ordering is semver's, so `0.2.0` > `0.2.0-beta.2` >
    // `0.2.0-beta.1` falls out for free, and a build ahead of everything
    // published — a local one, say — is simply up to date.
    if latest.version > *current {
        Ok(Status::Available(latest.clone()))
    } else {
        Ok(Status::UpToDate)
    }
}

/// The highest-versioned release `channel` is allowed to see.
///
/// Ordered by version rather than by publication date: re-publishing an older
/// release, or a patch to an old line landing after a new one, must not walk
/// users backwards.
pub fn latest_for(releases: &[Release], channel: Channel) -> Option<&Release> {
    releases
        .iter()
        .filter(|release| channel.accepts(release.channel))
        .max_by(|a, b| a.version.cmp(&b.version))
}

// ---------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------

fn fetch_releases(repo: &str) -> Result<String, UpdateError> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page={RELEASES_PER_PAGE}");

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();

    // Unauthenticated on purpose: the app holds no token and release metadata
    // is public. That caps us at GitHub's anonymous rate limit, which a check
    // per launch is nowhere near.
    let response = agent
        .get(&url)
        .header("User-Agent", concat!("drift/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();

    let mut response = match response {
        Ok(response) => response,
        // ureq reports a non-2xx as an error; the status is the useful part.
        Err(ureq::Error::StatusCode(status)) => return Err(UpdateError::Server(status)),
        Err(error) => return Err(UpdateError::Network(error.to_string())),
    };

    response
        .body_mut()
        .read_to_string()
        .map_err(|error| UpdateError::Network(error.to_string()))
}

/// The fields we read from GitHub's release objects. Everything else in the
/// payload — and there is a lot of it — is ignored, so the shape is allowed to
/// grow without breaking us.
#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
}

/// Turn GitHub's list into ours, dropping anything we cannot act on.
///
/// Skipping is the right response to a single odd entry: a hand-made tag like
/// `nightly` or an old `release-2024` should not stop the user from being told
/// about the perfectly good release next to it.
fn parse_releases(body: &str) -> Result<Vec<Release>, UpdateError> {
    let raw: Vec<ApiRelease> =
        serde_json::from_str(body).map_err(|error| UpdateError::Malformed(error.to_string()))?;

    Ok(raw.into_iter().filter_map(release_from_api).collect())
}

fn release_from_api(api: ApiRelease) -> Option<Release> {
    // Drafts are the maintainer's scratch space — visible only to them, and
    // by definition not yet released.
    if api.draft {
        return None;
    }

    let version = parse_tag(&api.tag_name)?;

    // Two independent signals, and either one is enough: GitHub's pre-release
    // flag, and a semver pre-release suffix. Trusting both means a release
    // published with the wrong flag still cannot reach a stable user, and a
    // `-beta` tag marked stable by mistake still sorts as a beta.
    let channel = if api.prerelease || !version.pre.is_empty() {
        Channel::Beta
    } else {
        Channel::Stable
    };

    Some(Release {
        dmg_url: pick_dmg(&api.assets),
        name: api
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| api.tag_name.clone()),
        notes: api.body.unwrap_or_default(),
        url: api.html_url,
        tag: api.tag_name,
        version,
        channel,
    })
}

/// `v0.2.0-beta.1` and `0.2.0-beta.1` both parse; anything else does not.
fn parse_tag(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().trim_start_matches('v')).ok()
}

/// The DMG for the architecture we are running on.
///
/// Releases may carry one DMG per architecture, a single universal one, or —
/// for a source-only release — none at all. Preference order is exact
/// architecture, then universal, then whatever DMG is there, so a release
/// built before the naming settled still offers a download.
fn pick_dmg(assets: &[ApiAsset]) -> Option<String> {
    let dmgs: Vec<&ApiAsset> = assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().ends_with(".dmg"))
        .collect();

    let aliases: Vec<&str> = match std::env::consts::ARCH {
        "aarch64" => vec!["aarch64", "arm64"],
        "x86_64" => vec!["x86_64", "x64", "intel", "amd64"],
        other => vec![other],
    };

    let named = |needles: &[&str]| {
        dmgs.iter().copied().find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            needles.iter().any(|needle| name.contains(needle))
        })
    };

    let chosen = named(&aliases)
        .or_else(|| named(&["universal"]))
        .or_else(|| dmgs.first().copied())?;

    Some(chosen.browser_download_url.clone())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool) -> Release {
        release_from_api(ApiRelease {
            tag_name: tag.to_owned(),
            name: None,
            body: None,
            draft: false,
            prerelease,
            html_url: format!("https://example.test/{tag}"),
            assets: Vec::new(),
        })
        .expect("test tags are parseable")
    }

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn a_tag_parses_with_or_without_its_v() {
        assert_eq!(parse_tag("v1.2.3"), Some(v("1.2.3")));
        assert_eq!(parse_tag("1.2.3"), Some(v("1.2.3")));
        assert_eq!(parse_tag(" v1.2.3 "), Some(v("1.2.3")));
        assert_eq!(parse_tag("v0.2.0-beta.1"), Some(v("0.2.0-beta.1")));

        // Tags that are not versions are not ours to interpret.
        assert_eq!(parse_tag("nightly"), None);
        assert_eq!(parse_tag("release-2024"), None);
        assert_eq!(parse_tag(""), None);
    }

    #[test]
    fn a_release_is_a_beta_if_either_signal_says_so() {
        assert_eq!(release("v1.0.0", false).channel, Channel::Stable);
        assert_eq!(release("v1.1.0-beta.1", false).channel, Channel::Beta);
        assert_eq!(release("v1.1.0", true).channel, Channel::Beta);
    }

    #[test]
    fn stable_subscribers_never_see_a_beta() {
        let releases = vec![
            release("v1.0.0", false),
            release("v1.1.0-beta.2", false),
            release("v0.9.0", false),
        ];

        let latest = latest_for(&releases, Channel::Stable).unwrap();
        assert_eq!(latest.version, v("1.0.0"));

        assert_eq!(
            evaluate(&releases, &v("1.0.0"), Channel::Stable).unwrap(),
            Status::UpToDate
        );
    }

    #[test]
    fn beta_subscribers_see_both_kinds_and_take_the_newest() {
        let releases = vec![release("v1.0.0", false), release("v1.1.0-beta.2", false)];

        let latest = latest_for(&releases, Channel::Beta).unwrap();
        assert_eq!(latest.version, v("1.1.0-beta.2"));

        // Once 1.1.0 ships, the beta line is behind it and the beta user moves
        // across rather than staying on betas forever.
        let releases = vec![
            release("v1.1.0", false),
            release("v1.1.0-beta.2", false),
            release("v1.0.0", false),
        ];
        let Status::Available(offered) =
            evaluate(&releases, &v("1.1.0-beta.2"), Channel::Beta).unwrap()
        else {
            panic!("1.1.0 is newer than the beta we are running");
        };
        assert_eq!(offered.version, v("1.1.0"));
        assert_eq!(offered.channel, Channel::Stable);
    }

    #[test]
    fn betas_are_ordered_among_themselves() {
        let releases = vec![
            release("v1.1.0-beta.9", false),
            release("v1.1.0-beta.10", false),
            release("v1.1.0-beta.2", false),
        ];
        // Numeric pre-release identifiers compare as numbers, so beta.10 beats
        // beta.9 — the trap a plain string sort falls into.
        assert_eq!(
            latest_for(&releases, Channel::Beta).unwrap().version,
            v("1.1.0-beta.10")
        );

        assert_eq!(
            evaluate(&releases, &v("1.1.0-beta.10"), Channel::Beta).unwrap(),
            Status::UpToDate
        );
    }

    #[test]
    fn a_prerelease_build_is_behind_its_own_final_release() {
        let releases = vec![release("v1.1.0", false)];
        let Status::Available(offered) =
            evaluate(&releases, &v("1.1.0-beta.1"), Channel::Stable).unwrap()
        else {
            panic!("the final 1.1.0 is newer than its beta");
        };
        assert_eq!(offered.version, v("1.1.0"));
    }

    #[test]
    fn a_build_ahead_of_every_release_is_up_to_date() {
        let releases = vec![release("v1.0.0", false)];
        assert_eq!(
            evaluate(&releases, &v("2.0.0"), Channel::Stable).unwrap(),
            Status::UpToDate
        );
    }

    #[test]
    fn a_channel_with_nothing_published_reports_no_releases() {
        // Only betas exist, so a stable user has nothing to compare against —
        // which is a different answer from "you are up to date".
        let releases = vec![release("v1.0.0-beta.1", true)];
        assert_eq!(
            evaluate(&releases, &v("0.9.0"), Channel::Stable),
            Err(UpdateError::NoReleases)
        );
        assert_eq!(
            evaluate(&[], &v("0.9.0"), Channel::Beta),
            Err(UpdateError::NoReleases)
        );
    }

    #[test]
    fn drafts_and_unparseable_tags_are_skipped_not_fatal() {
        let body = r#"[
            {"tag_name":"v2.0.0","draft":true,"prerelease":false,"html_url":"u"},
            {"tag_name":"nightly","draft":false,"prerelease":false,"html_url":"u"},
            {"tag_name":"v1.2.0","draft":false,"prerelease":false,"html_url":"u"}
        ]"#;
        let releases = parse_releases(body).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version, v("1.2.0"));
    }

    #[test]
    fn a_release_keeps_its_notes_title_and_page() {
        let body = r#"[{
            "tag_name":"v1.2.0","draft":false,"prerelease":false,
            "name":"Drift 1.2.0","body":"Stable release.\n\n## Features\n- Something",
            "html_url":"https://github.test/r/releases/tag/v1.2.0",
            "assets":[]
        }]"#;
        let release = &parse_releases(body).unwrap()[0];
        assert_eq!(release.name, "Drift 1.2.0");
        assert_eq!(release.notes, "Stable release.\n\n## Features\n- Something");
        assert_eq!(release.url, "https://github.test/r/releases/tag/v1.2.0");
        assert_eq!(release.tag, "v1.2.0");
        assert_eq!(release.display_version(), "1.2.0 (Stable)");
    }

    #[test]
    fn a_release_without_a_title_falls_back_to_its_tag() {
        let body = r#"[{"tag_name":"v1.2.0","name":"","draft":false,"prerelease":false}]"#;
        assert_eq!(parse_releases(body).unwrap()[0].name, "v1.2.0");
    }

    #[test]
    fn the_dmg_for_this_machine_is_preferred() {
        let asset = |name: &str| ApiAsset {
            name: name.to_owned(),
            browser_download_url: format!("https://example.test/{name}"),
        };

        // Non-DMG assets are never offered as a download.
        assert_eq!(pick_dmg(&[asset("Drift-1.0.0.tar.gz")]), None);
        assert_eq!(pick_dmg(&[]), None);

        let both = [
            asset("Drift-1.0.0-x86_64.dmg"),
            asset("Drift-1.0.0-aarch64.dmg"),
        ];
        let chosen = pick_dmg(&both).unwrap();
        let expected = match std::env::consts::ARCH {
            "aarch64" => "aarch64",
            _ => "x86_64",
        };
        assert!(chosen.contains(expected), "picked {chosen}");

        // A universal build serves any architecture...
        let universal = [asset("Drift-1.0.0-universal.dmg")];
        assert!(pick_dmg(&universal).unwrap().contains("universal"));

        // ...and an unlabelled DMG is better than telling the user there is
        // nothing to download.
        let plain = [asset("Drift.dmg")];
        assert!(pick_dmg(&plain).unwrap().ends_with("Drift.dmg"));
    }

    #[test]
    fn a_malformed_body_is_reported_not_panicked_on() {
        assert!(matches!(
            parse_releases("not json"),
            Err(UpdateError::Malformed(_))
        ));
        // A shape we did not expect — GitHub's error object, say — is the same
        // kind of answer: we cannot read it, and we say so.
        assert!(matches!(
            parse_releases(r#"{"message":"Not Found"}"#),
            Err(UpdateError::Malformed(_))
        ));
    }

    #[test]
    fn every_error_states_its_case_in_one_line() {
        for error in [
            UpdateError::Network("dns".into()),
            UpdateError::Server(403),
            UpdateError::Server(500),
            UpdateError::Malformed("eof".into()),
            UpdateError::NoReleases,
        ] {
            let message = error.to_string();
            assert!(!message.is_empty() && !message.contains('\n'), "{message}");
        }
    }

    #[test]
    fn the_running_build_reports_a_usable_version_and_channel() {
        let version = current_version();
        assert_eq!(version, Version::parse(env!("CARGO_PKG_VERSION")).unwrap());
        assert_eq!(current_channel(), Channel::of_version(&version));
    }

    /// Not part of the normal run: it needs the network, and a rate-limited
    /// GitHub would fail a suite that is otherwise about our own logic.
    ///
    ///   cargo test -- --ignored --nocapture
    ///
    /// Point it at a repository that publishes both kinds to see the channel
    /// rule hold against real metadata:
    ///
    ///   DRIFT_UPDATE_REPO=zed-industries/zed cargo test -- --ignored --nocapture
    #[test]
    #[ignore = "hits the GitHub API"]
    fn a_live_check_reaches_github() {
        println!("repository: {REPO}, running {}", current_version());

        for channel in Channel::ALL {
            match check(channel) {
                Ok(status) => {
                    println!("{}: {status:?}", channel.label());
                    // The invariant the whole feature rests on: whatever is
                    // published, a stable subscriber is never handed a beta.
                    if let (Channel::Stable, Status::Available(release)) = (channel, status) {
                        assert_eq!(release.channel, Channel::Stable);
                        assert!(release.version.pre.is_empty());
                    }
                }
                // Being offline, or rate limited, is a reported answer rather
                // than a panic — which is the property this really asserts.
                Err(error) => println!("{}: reported {error}", channel.label()),
            }
        }
    }

    #[test]
    fn channel_keys_round_trip_and_reject_junk() {
        for channel in Channel::ALL {
            assert_eq!(Channel::from_key(channel.key()), Some(channel));
        }
        assert_eq!(Channel::from_key("nightly"), None);
    }
}
