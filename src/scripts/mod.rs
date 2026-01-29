//! Module for various script formats and builders.
#[cfg(feature = "artemis")]
pub mod artemis;
pub mod base;
#[cfg(feature = "bgi")]
pub mod bgi;
#[cfg(feature = "cat-system")]
pub mod cat_system;
#[cfg(feature = "circus")]
pub mod circus;
#[cfg(feature = "emote-img")]
pub mod emote;
#[cfg(feature = "entis-gls")]
pub mod entis_gls;
#[cfg(feature = "escude")]
pub mod escude;
#[cfg(feature = "ex-hibit")]
pub mod ex_hibit;
#[cfg(feature = "favorite")]
pub mod favorite;
#[cfg(feature = "hexen-haus")]
pub mod hexen_haus;
#[cfg(feature = "kirikiri")]
pub mod kirikiri;
#[cfg(feature = "musica")]
pub mod musica;
#[cfg(feature = "qlie")]
pub mod qlie;
#[cfg(feature = "silky")]
pub mod silky;
#[cfg(feature = "softpal")]
pub mod softpal;
#[cfg(feature = "will-plus")]
pub mod will_plus;
#[cfg(feature = "yaneurao")]
pub mod yaneurao;

pub use base::{Script, ScriptBuilder};

lazy_static::lazy_static! {
    /// A list of all script builders.
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
        #[cfg(feature = "emote-img")]
        Box::new(emote::pimg::PImgBuilder::new()),
        #[cfg(feature = "emote-img")]
        Box::new(emote::dref::DrefBuilder::new()),
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
        #[cfg(feature = "circus-arc")]
        Box::new(circus::archive::pck::PckArchiveBuilder::new()),
        #[cfg(feature = "circus-audio")]
        Box::new(circus::audio::pcm::PcmBuilder::new()),
        #[cfg(feature = "ex-hibit")]
        Box::new(ex_hibit::rld::RldScriptBuilder::new()),
        #[cfg(feature = "circus-arc")]
        Box::new(circus::archive::dat::DatArchiveBuilder::new()),
        #[cfg(feature = "circus-arc")]
        Box::new(circus::archive::crm::CrmArchiveBuilder::new()),
        #[cfg(feature = "circus-img")]
        Box::new(circus::image::crxd::CrxdImageBuilder::new()),
        #[cfg(feature = "bgi-audio")]
        Box::new(bgi::audio::audio::BgiAudioBuilder::new()),
        #[cfg(feature = "entis-gls")]
        Box::new(entis_gls::srcxml::SrcXmlScriptBuilder::new()),
        #[cfg(feature = "softpal-arc")]
        Box::new(softpal::arc::pac::SoftpalPacBuilder::new()),
        #[cfg(feature = "softpal-arc")]
        Box::new(softpal::arc::pac::SoftpalPacBuilder::new_amuse()),
        #[cfg(feature = "softpal")]
        Box::new(softpal::scr::SoftpalScriptBuilder::new()),
        #[cfg(feature = "artemis-panmimisoft")]
        Box::new(artemis::panmimisoft::txt::TxtBuilder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::tjs_ns0::TjsNs0Builder::new()),
        #[cfg(feature = "kirikiri")]
        Box::new(kirikiri::tjs2::Tjs2Builder::new()),
        #[cfg(feature = "silky")]
        Box::new(silky::mes::MesBuilder::new()),
        #[cfg(feature = "favorite")]
        Box::new(favorite::hcb::HcbScriptBuilder::new()),
        #[cfg(feature = "silky")]
        Box::new(silky::map::MapBuilder::new()),
        #[cfg(feature = "emote-img")]
        Box::new(emote::psb::PsbBuilder::new()),
        #[cfg(feature = "softpal-img")]
        Box::new(softpal::img::pgd::ge::PgdGeBuilder::new()),
        #[cfg(feature = "softpal-img")]
        Box::new(softpal::img::pgd::pgd3::Pgd3Builder::new()),
        #[cfg(feature = "ex-hibit-arc")]
        Box::new(ex_hibit::arc::grp::ExHibitGrpArchiveBuilder::new()),
        #[cfg(feature = "hexen-haus-arc")]
        Box::new(hexen_haus::archive::arcc::HexenHausArccArchiveBuilder::new()),
        #[cfg(feature = "artemis-arc")]
        Box::new(artemis::archive::pf2::ArtemisPf2Builder::new()),
        #[cfg(feature = "hexen-haus-arc")]
        Box::new(hexen_haus::archive::wag::HexenHausWagArchiveBuilder::new()),
        #[cfg(feature = "hexen-haus-img")]
        Box::new(hexen_haus::img::png::PngImageBuilder::new()),
        #[cfg(feature = "hexen-haus-arc")]
        Box::new(hexen_haus::archive::odio::HexenHausOdioArchiveBuilder::new()),
        #[cfg(feature = "will-plus-img")]
        Box::new(will_plus::img::wip::WillPlusWipImageBuilder::new()),
        #[cfg(feature = "artemis")]
        Box::new(artemis::txt::ArtemisTxtBuilder::new()),
        #[cfg(feature = "kirikiri-arc")]
        Box::new(kirikiri::archive::xp3::Xp3ArchiveBuilder::new()),
        #[cfg(feature = "musica")]
        Box::new(musica::sc::MusicaBuilder::new()),
        #[cfg(feature = "musica-arc")]
        Box::new(musica::archive::paz::PazArcBuilder::new()),
        #[cfg(feature = "entis-gls")]
        Box::new(entis_gls::csx::CSXScriptBuilder::new()),
        #[cfg(feature = "qlie")]
        Box::new(qlie::script::QlieScriptBuilder::new()),
        #[cfg(feature = "qlie-arc")]
        Box::new(qlie::archive::pack::QliePackArchiveBuilder::new()),
    ];
    /// A list of all script extensions.
    pub static ref ALL_EXTS: Vec<String> =
        BUILDER.iter().flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
    /// A list of all script extensions that are archives.
    pub static ref ARCHIVE_EXTS: Vec<String> =
        BUILDER.iter().filter(|b| b.is_archive()).flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
}
