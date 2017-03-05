use net::*;
use net::clients::*;
use net::comms::*;

error_chain! {
    // The type defined for this error. These are the conventional
    // and recommended names, but they can be arbitrarily chosen.
    //
    // It is also possible to leave this section out entirely, or
    // leave it empty, and these names will be used automatically.
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    // Automatic conversions between this error chain and other
    // error types not defined by the `error_chain!`. These will be
    // wrapped in a new error with, in the first case, the
    // `ErrorKind::Fmt` variant. The description and cause will
    // forward to the description and cause of the original error.
    //
    // Optionally, some attributes can be added to a variant.
    //
    // This section can be empty.
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        Serde(::serde_json::Error);
        FutureMpscSendCmd(::futures::sync::mpsc::SendError<Cmd>);
        FutureMpscSendCommand(::futures::sync::mpsc::SendError<Command>);
        FutureMpscSendMsg(::futures::sync::mpsc::SendError<Msg>);
        FutureOneshot(::futures::sync::oneshot::Canceled);
        TokioTimer(::tokio_timer::TimerError);
    }

    // Define additional `ErrorKind` variants. The syntax here is
    // the same as `quick_error!`, but the `from()` and `cause()`
    // syntax is not supported.
    errors {
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
    }
}
