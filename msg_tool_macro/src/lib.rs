use proc_macro::TokenStream;
use syn::parse::discouraged::Speculative;
use syn::spanned::Spanned;

enum PackStruct {
    Enum(syn::ItemEnum),
    Struct(syn::ItemStruct),
}

impl syn::parse::Parse for PackStruct {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let fork = input.fork();

        // Try to parse as a struct first
        if let Ok(struct_item) = fork.parse::<syn::ItemStruct>() {
            // If successful, advance the original input stream and return
            input.advance_to(&fork);
            return Ok(PackStruct::Struct(struct_item));
        }

        // Try to parse as an enum
        if let Ok(enum_item) = input.parse::<syn::ItemEnum>() {
            return Ok(PackStruct::Enum(enum_item));
        }

        // If neither worked, create a helpful error
        Err(input.error("expected struct or enum"))
    }
}

#[proc_macro]
pub fn struct_unpack_impl_for_num(item: TokenStream) -> TokenStream {
    let i = syn::parse_macro_input!(item as syn::Ident);
    let output = quote::quote! {
        impl StructUnpack for #i {
            fn unpack<R: Read + Seek>(mut reader: R, big: bool, _encoding: Encoding) -> Result<Self> {
                let mut buf = [0u8; std::mem::size_of::<#i>()];
                reader.read_exact(&mut buf)?;
                Ok(if big {
                    #i::from_be_bytes(buf)
                } else {
                    #i::from_le_bytes(buf)
                })
            }
        }

        impl StructPack for #i {
            fn pack<W: Write>(&self, writer: &mut W, big: bool, _encoding: Encoding) -> Result<()> {
                let bytes = if big {
                    self.to_be_bytes()
                } else {
                    self.to_le_bytes()
                };
                writer.write_all(&bytes)?;
                Ok(())
            }
        }
    };
    output.into()
}

/// Macro to derive `StructPack` trait for structs.
/// 
/// make sure to import the necessary imports:
/// ```
/// use crate::ext::io::*;
/// use crate::utils::struct_pack::*;
/// use std::io::{Read, Seek, Write};
/// ```
///
/// * `skip_pack` attribute can be used to skip fields from packing.
/// * `fstring = <len>` attribute can be used to specify a fixed string length for String fields.
/// * `fstring_pad = <u8>` attribute can be used to specify a padding byte for fixed strings. (Default is 0)
/// * `fvec = <len>` attribute can be used to specify a fixed vector length for Vec<_> fields.
/// * `pstring(<len_type>)` attribute can be used to specify a packed string length for String fields, where `<len_type>` can be `u8`, `u16`, `u32`, or `u64`.
/// Length is read as a prefix before the string data.
#[proc_macro_derive(StructPack, attributes(skip_pack, fstring, fstring_pad, fvec, pstring))]
pub fn struct_pack_derive(input: TokenStream) -> TokenStream {
    let a = syn::parse_macro_input!(input as PackStruct);
    match a {
        PackStruct::Struct(sut) => {
            let name = sut.ident;
            let mut ind = 0;
            let fields = sut.fields.iter().map(|field| {
                let mut skipped = false;
                let mut fixed_string: Option<usize> = None;
                let mut fixed_vec: Option<usize> = None;
                let mut fstring_pad = 0u8; // Default padding byte
                let mut pstring_type: Option<syn::Ident> = None;
                for attr in &field.attrs {
                    let path = attr.path();
                    if path.is_ident("skip_pack") {
                        skipped = true;
                    } else if path.is_ident("fstring") {
                        if let syn::Meta::NameValue(nv) = &attr.meta {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let syn::Lit::Int(s) = &lit.lit {
                                    fixed_string = Some(s.base10_parse().unwrap());
                                }
                            }
                        }
                    } else if path.is_ident("fvec") {
                        if let syn::Meta::NameValue(nv) = &attr.meta {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let syn::Lit::Int(s) = &lit.lit {
                                    fixed_vec = Some(s.base10_parse().unwrap());
                                }
                            }
                        }
                    } else if path.is_ident("fstring_pad") {
                        if let syn::Meta::NameValue(nv) = &attr.meta {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let syn::Lit::Int(s) = &lit.lit {
                                    fstring_pad = s.base10_parse().unwrap();
                                }
                            }
                        }
                    } else if path.is_ident("pstring") {
                        if let syn::Meta::List(list) = &attr.meta {
                            list.parse_nested_meta(|meta| {
                                if meta.path.is_ident("u8") {
                                    pstring_type = Some(syn::Ident::new("u8", meta.path.span()));
                                } else if meta.path.is_ident("u16") {
                                    pstring_type = Some(syn::Ident::new("u16", meta.path.span()));
                                } else if meta.path.is_ident("u32") {
                                    pstring_type = Some(syn::Ident::new("u32", meta.path.span()));
                                } else if meta.path.is_ident("u64") {
                                    pstring_type = Some(syn::Ident::new("u64", meta.path.span()));
                                } else {
                                    return Err(meta.error("Expected u8, u16, or u32 for pstring"));
                                }
                                Ok(())
                            }).unwrap();
                        }
                    }
                }
                if skipped {
                    return quote::quote! {};
                }
                let field_name = match &field.ident {
                    Some(ident) => quote::quote! { #ident },
                    None => {
                        let idx = syn::Index::from(ind);
                        ind += 1;
                        quote::quote! { #idx }
                    },
                };
                let field_type = &field.ty;
                if let syn::Type::Path(type_path) = field_type {
                    if let Some(segment) = type_path.path.segments.last() {
                        if segment.ident == "String" {
                            if let Some(fixed_string) = fixed_string {
                                return quote::quote! {
                                    let s = encode_string(encoding, &self.#field_name, true)?;
                                    let mut slen = s.len();
                                    if slen > #fixed_string {
                                        return Err(anyhow::anyhow!("String length was too long for field '{}'", stringify!(#field_name)));
                                    }
                                    writer.write_all(&s)?;
                                    if slen < #fixed_string {
                                        writer.write_all(&[0])?;
                                        slen += 1;
                                    }
                                    for _ in slen..#fixed_string {
                                        writer.write_all(&[#fstring_pad])?;
                                    }
                                };
                            }
                            if let Some(pstring_type) = pstring_type {
                                let write_fn = syn::Ident::new(format!("write_{}", pstring_type).as_str(), pstring_type.span());
                                return quote::quote! {
                                    let encoded = crate::utils::encoding::encode_string(encoding, &self.#field_name, true)?;
                                    writer.#write_fn(encoded.len() as #pstring_type)?;
                                    writer.write_all(&encoded)?;
                                };
                            }
                        }
                    }
                    if let Some(segment) = type_path.path.segments.first() {
                        if segment.ident == "Vec" {
                            if let Some(fixed_vec) = fixed_vec {
                                return quote::quote! {
                                    if self.#field_name.len() != #fixed_vec {
                                        return Err(anyhow::anyhow!("Vector length was not equal to {}", #fixed_vec));
                                    }
                                    for item in &self.#field_name {
                                        item.pack(writer, big, encoding)?;
                                    }
                                };
                            }
                        }
                    }
                }
                quote::quote! {
                    self.#field_name.pack(writer, big, encoding)?;
                }
            });
            let output = quote::quote! {
                impl StructPack for #name {
                    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
                        #(#fields)*
                        Ok(())
                    }
                }
            };
            output.into()
        }
        PackStruct::Enum(item) => {
            let ident = item.ident;
            let variants = item.variants.iter().map(|variant| {
                let mut skipped = false;
                for attr in &variant.attrs {
                    let path = attr.path();
                    if path.is_ident("skip_pack") {
                        skipped = true;
                    }
                }
                if skipped {
                    return quote::quote! {};
                }
                let variant_name = &variant.ident;
                let mut idents = Vec::new();
                let mut is_struct_like = true;
                let fields: Vec<_> = variant.fields.iter().enumerate().map(|(idx, field)| {
                    let mut skipped = false;
                    let mut fixed_string: Option<usize> = None;
                    let mut fixed_vec: Option<usize> = None;
                    let mut fstring_pad = 0u8; // Default padding byte
                    for attr in &field.attrs {
                        let path = attr.path();
                        if path.is_ident("skip_pack") {
                            skipped = true;
                        } else if path.is_ident("fstring") {
                            if let syn::Meta::NameValue(nv) = &attr.meta {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let syn::Lit::Int(s) = &lit.lit {
                                        fixed_string = Some(s.base10_parse().unwrap());
                                    }
                                }
                            }
                        } else if path.is_ident("fvec") {
                            if let syn::Meta::NameValue(nv) = &attr.meta {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let syn::Lit::Int(s) = &lit.lit {
                                        fixed_vec = Some(s.base10_parse().unwrap());
                                    }
                                }
                            }
                        } else if path.is_ident("fstring_pad") {
                            if let syn::Meta::NameValue(nv) = &attr.meta {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let syn::Lit::Int(s) = &lit.lit {
                                        fstring_pad = s.base10_parse().unwrap();
                                    }
                                }
                            }
                        }
                    }
                    if skipped {
                        return quote::quote! {};
                    }
                    let field_name = match &field.ident {
                        Some(ident) => quote::quote! { #ident },
                        None => {
                            is_struct_like = false;
                            let idx = syn::Ident::new(&format!("index_{}", idx), field.span());
                            quote::quote! { #idx }
                        },
                    };
                    idents.push(field_name.clone());
                    let field_type = &field.ty;
                    if let syn::Type::Path(type_path) = field_type {
                        if let Some(segment) = type_path.path.segments.last() {
                            if segment.ident == "String" {
                                if let Some(fixed_string) = fixed_string {
                                    return quote::quote! {
                                        let s = encode_string(encoding, &#field_name, true)?;
                                        let mut slen = s.len();
                                        if slen > #fixed_string {
                                            return Err(anyhow::anyhow!("String length was too long for field '{}'", stringify!(#field_name)));
                                        }
                                        writer.write_all(&s)?;
                                        if slen < #fixed_string {
                                            writer.write_all(&[0])?;
                                            slen += 1;
                                        }
                                        for _ in slen..#fixed_string {
                                            writer.write_all(&[#fstring_pad])?;
                                        }
                                    };
                                }
                            }
                        }
                        if let Some(segment) = type_path.path.segments.first() {
                            if segment.ident == "Vec" {
                                if let Some(fixed_vec) = fixed_vec {
                                    return quote::quote! {
                                        if #field_name.len() != #fixed_vec {
                                            return Err(anyhow::anyhow!("Vector length was not equal to {}", #fixed_vec));
                                        }
                                        for item in &#field_name {
                                            item.pack(writer, big, encoding)?;
                                        }
                                    };
                                }
                            }
                        }
                    }
                    quote::quote! {
                        #field_name.pack(writer, big, encoding)?;
                    }
                }).collect();
                let idents = if is_struct_like {
                    quote::quote! { { #(#idents),* } }
                } else {
                    quote::quote! { (#(#idents),*) }
                };
                quote::quote! {
                    #ident::#variant_name #idents => {
                        #(#fields)*
                    }
                }
            });
            let output = quote::quote! {
                impl StructPack for #ident {
                    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
                        match self {
                            #(#variants)*
                        }
                        Ok(())
                    }
                }
            };
            output.into()
        }
    }
}

/// Macro to derive `StructUnpack` trait for structs.
/// 
/// make sure to import the necessary imports:
/// ```
/// use crate::ext::io::*;
/// use crate::utils::struct_pack::*;
/// use std::io::{Read, Seek, Write};
/// ```
/// 
/// * `skip_unpack` attribute can be used to skip fields from unpacking.
/// * `fstring = <len>` attribute can be used to specify a fixed string length for String fields.
/// * `fstring_no_trim` attribute can be used to disable trimming of fixed strings.
/// * `fvec = <len>` attribute can be used to specify a fixed vector length for Vec<_> fields.
/// * `pstring(<number_type>)` attribute can be used to specify a packed string length for String fields, where `<number_type>` can be `u8`, `u16`, `u32` or `u64`.
/// length is read as a prefix before the string data.
#[proc_macro_derive(StructUnpack, attributes(skip_unpack, fstring, fstring_no_trim, fvec, pstring))]
pub fn struct_unpack_derive(input: TokenStream) -> TokenStream {
    let sut = syn::parse_macro_input!(input as syn::ItemStruct);
    let name = sut.ident;
    let mut fields = Vec::new();
    let mut is_tuple_struct = false;
    let mut ind = 0;
    let smts: Vec<_> = sut.fields.iter().map(|field| {
        let mut skipped = false;
        let mut fixed_string: Option<usize> = None;
        let mut fstring_no_trim = false;
        let mut fixed_vec: Option<usize> = None;
        let mut pstring_type: Option<syn::Ident> = None;
        for attr in &field.attrs {
            let path = attr.path();
            if path.is_ident("skip_unpack") {
                skipped = true;
            } else if path.is_ident("fstring") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(lit) = &nv.value {
                        if let syn::Lit::Int(s) = &lit.lit {
                            fixed_string = Some(s.base10_parse().unwrap());
                        }
                    }
                }
            } else if path.is_ident("fstring_no_trim") {
                fstring_no_trim = true;
            } else if path.is_ident("fvec") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(lit) = &nv.value {
                        if let syn::Lit::Int(s) = &lit.lit {
                            fixed_vec = Some(s.base10_parse().unwrap());
                        }
                    }
                }
            } else if path.is_ident("pstring") {
                if let syn::Meta::List(list) = &attr.meta {
                    list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("u8") {
                            pstring_type = Some(syn::Ident::new("u8", meta.path.span()));
                        } else if meta.path.is_ident("u16") {
                            pstring_type = Some(syn::Ident::new("u16", meta.path.span()));
                        } else if meta.path.is_ident("u32") {
                            pstring_type = Some(syn::Ident::new("u32", meta.path.span()));
                        } else if meta.path.is_ident("u64") {
                            pstring_type = Some(syn::Ident::new("u64", meta.path.span()));
                        } else {
                            return Err(meta.error("Expected u8, u16, or u32 for pstring"));
                        }
                        Ok(())
                    }).unwrap();
                }
            }
        }
        let field_name = match &field.ident {
            Some(ident) => quote::quote! { #ident },
            None => {
                is_tuple_struct = true;
                let idx = syn::Ident::new(&format!("index_{}", ind), field.span());
                ind += 1;
                quote::quote! { #idx }
            },
        };
        fields.push(field_name.clone());
        if skipped {
            return quote::quote! {
                let #field_name = Default::default();
            };
        }
        let field_type = &field.ty;
        if let syn::Type::Path(type_path) = field_type {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "String" {
                    if let Some(fixed_string) = fixed_string {
                        let trim = syn::LitBool::new(!fstring_no_trim, field.span());
                        return quote::quote! {
                            let #field_name = reader.read_fstring(#fixed_string, encoding, #trim)?;
                        };
                    }
                    if let Some(pstring_type) = pstring_type {
                        let read_fn = syn::Ident::new(format!("read_{}", pstring_type).as_str(), pstring_type.span());
                        return quote::quote! {
                            let len = reader.#read_fn()? as usize;
                            let #field_name = reader.read_exact_vec(len)?;
                            let #field_name = crate::utils::encoding::decode_to_string(encoding, &#field_name, true)?;
                        }
                    }
                }
            }
            if let Some(segment) = type_path.path.segments.first() {
                if segment.ident == "Vec" {
                    if let Some(fixed_vec) = fixed_vec {
                        return quote::quote! {
                            let #field_name = reader.read_struct_vec(#fixed_vec, big, encoding)?;
                        };
                    }
                }
            }
        }
        quote::quote! {
            let #field_name = #field_type::unpack(&mut reader, big, encoding)?;
        }
    }).collect();
    let fields = if is_tuple_struct {
        quote::quote! ((#(#fields),*))
    } else {
        quote::quote! { { #(#fields),* } }
    };
    let output = quote::quote! {
        impl StructUnpack for #name {
            fn unpack<R: Read + Seek>(mut reader: R, big: bool, encoding: Encoding) -> Result<Self> {
                #(#smts)*
                Ok(Self #fields)
            }
        }
    };
    output.into()
}

#[cfg(feature = "artemis-arc")]
#[proc_macro]
pub fn gen_artemis_arc_ext(_: TokenStream) -> TokenStream {
    let mut exts = Vec::new();
    exts.push(quote::quote! { "pfs" });
    for i in 0..=999 {
        let ext = format!("pfs.{:03}", i);
        exts.push(quote::quote! { #ext });
    }
    let output = quote::quote! {
        &[
            #(#exts),*
        ]
    };
    output.into()
}
