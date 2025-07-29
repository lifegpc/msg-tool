#[cfg(feature = "artemis")]
pub mod artemis;
pub mod base;
#[cfg(feature = "bgi")]
pub mod bgi;
#[cfg(feature = "cat-system")]
pub mod cat_system;
#[cfg(feature = "circus")]
pub mod circus;
#[cfg(feature = "escude")]
pub mod escude;
#[cfg(feature = "hexen-haus")]
pub mod hexen_haus;
#[cfg(feature = "kirikiri")]
pub mod kirikiri;
#[cfg(feature = "will-plus")]
pub mod will_plus;
#[cfg(feature = "yaneurao")]
pub mod yaneurao;

pub use base::{Script, ScriptBuilder};

lazy_static::lazy_static! {
    pub static ref BUILDER: Vec<Box<dyn ScriptBuilder + Sync + Send>> = vec![
        #[cfg(feature = "circus")]
        Box::new(circus::script::CircusMesScriptBuilder::new()),
        #[cfg(feature = "bgi")]
        Box::new(bgi::script::BGIScriptBuilder::new()),
        #[cfg(feature = "bgi")]
        Box::new(bgi::bsi::BGIBsiScriptBuilder::new()),
        #[cfg(feature = "bgi")]
        Box::new(bgi::bp::BGIBpScriptBuilder::new()),
        #[cfg(feature = "bgi-arc")]
        Box::new(bgi::archive::v1::BgiArchiveBuilder::new()),
        #[cfg(feature = "bgi-arc")]
        Box::new(bgi::archive::v2::BgiArchiveBuilder::new()),
        #[cfg(feature = "bgi-arc")]
        Box::new(bgi::archive::dsc::DscBuilder::new()),
        #[cfg(feature = "bgi-img")]
        Box::new(bgi::image::img::BgiImageBuilder::new()),
        #[cfg(feature = "bgi-img")]
        Box::new(bgi::image::cbg::BgiCBGBuilder::new()),
        #[cfg(feature = "escude-arc")]
        Box::new(escude::archive::EscudeBinArchiveBuilder::new()),
        #[cfg(feature = "escude")]
        Box::new(escude::script::EscudeBinScriptBuilder::new()),
        #[cfg(feature = "escude")]
        Box::new(escude::list::EscudeBinListBuilder::new()),
        #[cfg(feature = "yaneurao-itufuru")]
        Box::new(yaneurao::itufuru::script::ItufuruScriptBuilder::new()),
        #[cfg(feature = "yaneurao-itufuru")]
        Box::new(yaneurao::itufuru::archive::ItufuruArchiveBuilder::new()),
        #[cfg(feature = "cat-system-arc")]
        Box::new(cat_system::archive::int::CSIntArcBuilder::new()),
        #[cfg(feature = "cat-system-img")]
        Box::new(cat_system::image::hg3::Hg3ImageBuilder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::scn::ScnScriptBuilder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::simple_crypt::SimpleCryptBuilder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::ks::KsBuilder::new()),
        #[cfg(feature = "kirikiri-img")]
        Box::new(kirikiri::image::tlg::TlgImageBuilder::new()),
        #[cfg(feature = "kirikiri-img")]
        Box::new(kirikiri::image::pimg::PImgBuilder::new()),
        #[cfg(feature = "kirikiri-img")]
        Box::new(kirikiri::image::dref::DrefBuilder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::mdf::MdfBuilder::new()),
        #[cfg(feature = "will-plus")]
        Box::new(will_plus::ws2::Ws2ScriptBuilder::new()),
        #[cfg(feature = "cat-system")]
        Box::new(cat_system::cst::CstScriptBuilder::new()),
        #[cfg(feature = "artemis-arc")]
        Box::new(artemis::archive::pfs::ArtemisArcBuilder::new()),
        #[cfg(feature = "artemis")]
        Box::new(artemis::ast::AstScriptBuilder::new()),
        #[cfg(feature = "artemis")]
        Box::new(artemis::asb::ArtemisAsbBuilder::new()),
        #[cfg(feature = "hexen-haus")]
        Box::new(hexen_haus::bin::BinScriptBuilder::new()),
        #[cfg(feature = "circus-img")]
        Box::new(circus::image::crx::CrxImageBuilder::new()),
        #[cfg(feature = "cat-system")]
        Box::new(cat_system::cstl::CstlScriptBuilder::new()),
    ];
    pub static ref ALL_EXTS: Vec<String> =
        BUILDER.iter().flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
    pub static ref ARCHIVE_EXTS: Vec<String> =
        BUILDER.iter().filter(|b| b.is_archive()).flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
}
