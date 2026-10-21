//! Apollo asset source: kit defaults plus the Lucide icons the default
//! bundle omits.
//!
//! `gpui_kit::assets::Assets` embeds only ~106 icons. Anything else used via
//! `IconName` (e.g. `Terminal` for the bash tool row) silently renders
//! nothing. This source serves our extras first and falls back to the kit
//! defaults, keeping the binary far smaller than `AllAssets`.

use std::borrow::Cow;

use gpui_kit::assets::{Assets, icon_assets};
use gpui_kit::*;

icon_assets!(
    pub ApolloExtraAssets,
    [
        Terminal,
        SquarePen,
        Pencil,
        Wrench,
        MessageCircle,
        Sparkles,
        Zap,
        X,
        CornerDownLeft
    ]
);

#[derive(Clone, Copy, Debug, Default)]
pub struct ApolloAssets;

impl AssetSource for ApolloAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(data) = ApolloExtraAssets.load(path)? {
            return Ok(Some(data));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = ApolloExtraAssets.list(path)?;
        out.extend(Assets.list(path)?);
        Ok(out)
    }
}
