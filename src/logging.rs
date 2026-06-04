use std::time::Instant;

use rquickjs::{Ctx, Function, Object, Result, Value};

const PROFILE_MIN_ELAPSED_MS: u128 = 0;

pub fn profile_image_operation(ctx: &Ctx<'_>, label: &str, started_at: Instant, details: String) {
    let elapsed_ms = started_at.elapsed().as_millis();
    if elapsed_ms < PROFILE_MIN_ELAPSED_MS {
        return;
    }

    debug_image(
        ctx,
        format!("[gagaku:image] native {label} {elapsed_ms}ms {details}"),
    );
}

pub fn log_image_error(ctx: &Ctx<'_>, label: &str, started_at: Instant, details: String) {
    let elapsed_ms = started_at.elapsed().as_millis();
    debug_image(
        ctx,
        format!("[gagaku:image] native {label} failed {elapsed_ms}ms {details}"),
    );
}

fn debug_image(ctx: &Ctx<'_>, message: String) {
    let Ok(console) = ctx.globals().get::<_, Object<'_>>("console") else {
        return;
    };
    let Ok(debug) = console.get::<_, Function<'_>>("debug") else {
        return;
    };
    let _: Result<Value<'_>> = debug.call((message,));
}
