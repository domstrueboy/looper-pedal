use crate::config::AppConfig;

/// What a rendered frame asks the app to do next.
///
/// Lives down here rather than with the app because both sides need it:
/// the screens return it and the shell acts on it. Putting it with the
/// shell would have the screens depending on the thing that calls them.
pub enum Action {
    Start(AppConfig),
    OpenSettings,
}
