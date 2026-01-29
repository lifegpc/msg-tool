#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_cfg))]
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
/// Implements `StructUnpack` and `StructPack` traits for numeric types.
pub fn struct_unpack_impl_for_num(item: TokenStream) -> TokenStream {
    let i = syn::parse_macro_input!(item as syn::Ident);
    let output = quote::quote! {
        impl StructUnpack for #i {
            fn unpack<R: Read + Seek>(reader: &mut R, big: bool, _encoding: Encoding, _info: &Option<Box<dyn std::any::Any>>) -> Result<Self> {
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
            fn pack<W: Write>(&self, writer: &mut W, big: bool, _encoding: Encoding, _info: &Option<Box<dyn std::any::Any>>) -> Result<()> {
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
/// * `cstring` attribute can be used to specify that a String field is a C-style string (null-terminated).
/// * `fstring = <len>` attribute can be used to specify a fixed string length for String fields.
/// * `fstring_pad = <u8>` attribute can be used to specify a padding byte for fixed strings. (Default is 0)
/// * `fvec = <len>` attribute can be used to specify a fixed vector length for Vec<_> fields.
/// * `pstring(<len_type>)` attribute can be used to specify a packed string length for String fields, where `<len_type>` can be `u8`, `u16`, `u32`, or `u64`.
/// Length is read as a prefix before the string data.
/// * `pvec(<len_type>)` attribute can be used to specify a packed vector length for Vec<_> fields, where `<len_type>` can be `u8`, `u16`, `u32`, or `u64`.
/// Length is read as a prefix before the vector data.
/// * `skip_pack_if(<expr>)` attribute can be used to skip packing a field if the expression evaluates to true. The expression must be a valid Rust expression that evaluates to a boolean.
/// * `pack_vec_len(<expr>)` can be used to specify the length of a Vec field based on an expression. The expression must evaluate to a usize and can reference previously packed fields.
#[proc_macro_derive(
    StructPack,
    attributes(
        skip_pack,
        cstring,
        fstring,
        fstring_pad,
        fvec,
        pstring,
        pvec,
        skip_pack_if,
        pack_vec_len,
    )
)]
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
                let mut pvec_type: Option<syn::Ident> = None;
                let mut cur = None;
                let mut skip_if = None;
                let mut is_cstring = false;
                let mut vec_len: Option<syn::Expr> = None;
                for attr in &field.attrs {
                    let path = attr.path();
                    if path.is_ident("skip_pack") {
                        skipped = true;
                    } else if path.is_ident("cstring") {
                        is_cstring = true;
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
                    } else if path.is_ident("pvec") {
                        if let syn::Meta::List(list) = &attr.meta {
                            list.parse_nested_meta(|meta| {
                                if meta.path.is_ident("u8") {
                                    pvec_type = Some(syn::Ident::new("u8", meta.path.span()));
                                } else if meta.path.is_ident("u16") {
                                    pvec_type = Some(syn::Ident::new("u16", meta.path.span()));
                                } else if meta.path.is_ident("u32") {
                                    pvec_type = Some(syn::Ident::new("u32", meta.path.span()));
                                } else if meta.path.is_ident("u64") {
                                    pvec_type = Some(syn::Ident::new("u64", meta.path.span()));
                                } else {
                                    return Err(meta.error("Expected u8, u16, or u32 for pvec"));
                                }
                                Ok(())
                            }).unwrap();
                        }
                    } else if path.is_ident("skip_pack_if") {
                        if let syn::Meta::List(list) = &attr.meta {
                            skip_if = Some(list.parse_args::<syn::Expr>().unwrap());
                        }
                    } else if path.is_ident("pack_vec_len") {
                        if let syn::Meta::List(list) = &attr.meta {
                            vec_len = Some(list.parse_args::<syn::Expr>().unwrap());
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
                            if is_cstring {
                                cur = Some(quote::quote! {
                                    let s = encode_string(encoding, &self.#field_name, true)?;
                                    writer.write_all(&s)?;
                                    writer.write_all(&[0])?;
                                });
                            } else if let Some(fixed_string) = fixed_string {
                                cur = Some(quote::quote! {
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
                                });
                            } else if let Some(pstring_type) = pstring_type {
                                cur = Some(quote::quote! {
                                    let encoded = crate::utils::encoding::encode_string(encoding, &self.#field_name, true)?;
                                    let len = encoded.len() as #pstring_type;
                                    len.pack(writer, big, encoding, __info)?;
                                    writer.write_all(&encoded)?;
                                });
                            }
                        }
                    }
                    if let Some(segment) = type_path.path.segments.first() {
                        if segment.ident == "Vec" {
                            if let Some(fixed_vec) = fixed_vec {
                                cur = Some(quote::quote! {
                                    if self.#field_name.len() != #fixed_vec {
                                        return Err(anyhow::anyhow!("Vector length was not equal to {}", #fixed_vec));
                                    }
                                    for item in &self.#field_name {
                                        item.pack(writer, big, encoding, __info)?;
                                    }
                                });
                            } else if let Some(pvec_type) = pvec_type {
                                cur = Some(quote::quote! {
                                    let len = self.#field_name.len() as #pvec_type;
                                    len.pack(writer, big, encoding, __info)?;
                                    for item in &self.#field_name {
                                        item.pack(writer, big, encoding, __info)?;
                                    }
                                });
                            } else if let Some(vec_len) = vec_len {
                                cur = Some(quote::quote! {
                                    let len = (#vec_len) as usize;
                                    if self.#field_name.len() != len {
                                        return Err(anyhow::anyhow!("Vector length was not equal to the specified length for field '{}'", stringify!(#field_name)));
                                    }
                                    for item in &self.#field_name {
                                        item.pack(writer, big, encoding, __info)?;
                                    }
                                });
                            }
                        }
                    }
                }
                let p = cur.unwrap_or_else(|| {
                    quote::quote! {
                        self.#field_name.pack(writer, big, encoding, __info)?;
                    }
                });
                if let Some(skip_if) = skip_if {
                    quote::quote! {
                        if !(#skip_if) {
                            #p
                        }
                    }
                } else {
                    p
                }
            });
            let output = quote::quote! {
                impl StructPack for #name {
                    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding, __info: &Option<Box<dyn std::any::Any>>) -> Result<()> {
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
                    let mut pstring_type: Option<syn::Ident> = None;
                    let mut pvec_type: Option<syn::Ident> = None;
                    let mut cur = None;
                    let mut skip_if = None;
                    let mut is_cstring = false;
                    let mut vec_len: Option<syn::Expr> = None;
                    for attr in &field.attrs {
                        let path = attr.path();
                        if path.is_ident("skip_pack") {
                            skipped = true;
                        } else if path.is_ident("cstring") {
                            is_cstring = true;
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
                        } else if path.is_ident("pvec") {
                            if let syn::Meta::List(list) = &attr.meta {
                                list.parse_nested_meta(|meta| {
                                    if meta.path.is_ident("u8") {
                                        pvec_type = Some(syn::Ident::new("u8", meta.path.span()));
                                    } else if meta.path.is_ident("u16") {
                                        pvec_type = Some(syn::Ident::new("u16", meta.path.span()));
                                    } else if meta.path.is_ident("u32") {
                                        pvec_type = Some(syn::Ident::new("u32", meta.path.span()));
                                    } else if meta.path.is_ident("u64") {
                                        pvec_type = Some(syn::Ident::new("u64", meta.path.span()));
                                    } else {
                                        return Err(meta.error("Expected u8, u16, or u32 for pvec"));
                                    }
                                    Ok(())
                                }).unwrap();
                            }
                        } else if path.is_ident("skip_pack_if") {
                            if let syn::Meta::List(list) = &attr.meta {
                                skip_if = Some(list.parse_args::<syn::Expr>().unwrap());
                            }
                        } else if path.is_ident("pack_vec_len") {
                            if let syn::Meta::List(list) = &attr.meta {
                                vec_len = Some(list.parse_args::<syn::Expr>().unwrap());
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
                                if is_cstring {
                                    cur = Some(quote::quote! {
                                        let s = encode_string(encoding, &#field_name, true)?;
                                        writer.write_all(&s)?;
                                        writer.write_all(&[0])?;
                                    });
                                } else if let Some(fixed_string) = fixed_string {
                                    cur = Some(quote::quote! {
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
                                    });
                                } else if let Some(pstring_type) = pstring_type {
                                    cur = Some(quote::quote! {
                                        let encoded = crate::utils::encoding::encode_string(encoding, &#field_name, true)?;
                                        let len = encoded.len() as #pstring_type;
                                        len.pack(writer, big, encoding, __info)?;
                                        writer.write_all(&encoded)?;
                                    });
                                }
                            }
                        }
                        if let Some(segment) = type_path.path.segments.first() {
                            if segment.ident == "Vec" {
                                if let Some(fixed_vec) = fixed_vec {
                                    cur = Some(quote::quote! {
                                        if #field_name.len() != #fixed_vec {
                                            return Err(anyhow::anyhow!("Vector length was not equal to {}", #fixed_vec));
                                        }
                                        for item in &#field_name {
                                            item.pack(writer, big, encoding, __info)?;
                                        }
                                    });
                                } else if let Some(pvec_type) = pvec_type {
                                    cur = Some(quote::quote! {
                                        let len = #field_name.len() as #pvec_type;
                                        len.pack(writer, big, encoding, __info)?;
                                        for item in &#field_name {
                                            item.pack(writer, big, encoding, __info)?;
                                        }
                                    });
                                } else if let Some(vec_len) = vec_len {
                                    cur = Some(quote::quote! {
                                        let len = (#vec_len) as usize;
                                        if #field_name.len() != len {
                                            return Err(anyhow::anyhow!("Vector length was not equal to the specified length for field '{}'", stringify!(#field_name)));
                                        }
                                        for item in &#field_name {
                                            item.pack(writer, big, encoding, __info)?;
                                        }
                                    });
                                }
                            }
                        }
                    }
                    let p = cur.unwrap_or_else(|| {
                        quote::quote! {
                            #field_name.pack(writer, big, encoding, __info)?;
                        }
                    });
                    if let Some(skip_if) = skip_if {
                        quote::quote! {
                            if !(#skip_if) {
                                #p
                            }
                        }
                    } else {
                        p
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
                    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding, __info: &Option<Box<dyn std::any::Any>>) -> Result<()> {
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
/// * `cstring` attribute can be used to specify that a String field is a C-style string (null-terminated).
/// * `fstring = <len>` attribute can be used to specify a fixed string length for String fields.
/// * `fstring_no_trim` attribute can be used to disable trimming of fixed strings.
/// * `fvec = <len>` attribute can be used to specify a fixed vector length for Vec<_> fields.
/// * `pstring(<number_type>)` attribute can be used to specify a packed string length for String fields, where `<number_type>` can be `u8`, `u16`, `u32` or `u64`.
/// length is read as a prefix before the string data.
/// * `pvec(<number_type>)` attribute can be used to specify a packed vector length for Vec<_> fields, where `<number_type>` can be `u8`, `u16`, `u32` or `u64`.
/// length is read as a prefix before the vector data.
/// * `skip_unpack_if(<expr>)` attribute can be used to skip unpacking a field if the expression evaluates to true. The expression must be a valid Rust expression that evaluates to a boolean.
/// * `pvec_max_preallocated_size(<size>)` attribute can be used to limit the maximum preallocated size for packed vectors to prevent excessive memory allocation. If the unpacked size exceeds this limit, preallocation will be disabled. Default size is 0x400000 (4 MB).
/// * `unpack_vec_len(<expr>)` can be used to specify the length of a Vec field based on an expression. The expression must evaluate to a usize and can reference previously unpacked fields.
#[proc_macro_derive(
    StructUnpack,
    attributes(
        skip_unpack,
        cstring,
        fstring,
        fstring_no_trim,
        fvec,
        pstring,
        pvec,
        skip_unpack_if,
        pvec_max_preallocated_size,
        unpack_vec_len,
    )
)]
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
        let mut pvec_type: Option<syn::Ident> = None;
        let mut cur = None;
        let mut skip_if: Option<syn::Expr> = None;
        let mut is_cstring = false;
        let mut vec_len : Option<syn::Expr> = None;
        let mut pvec_max_preallocated_size = syn::Expr::Lit(syn::ExprLit {
            attrs: Vec::new(),
            lit: syn::Lit::Int(syn::LitInt::new("4194304", field.span())), // Default 4 MB
        });
        for attr in &field.attrs {
            let path = attr.path();
            if path.is_ident("skip_unpack") {
                skipped = true;
            } else if path.is_ident("cstring") {
                is_cstring = true;
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
            } else if path.is_ident("pvec") {
                if let syn::Meta::List(list) = &attr.meta {
                    list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("u8") {
                            pvec_type = Some(syn::Ident::new("u8", meta.path.span()));
                        } else if meta.path.is_ident("u16") {
                            pvec_type = Some(syn::Ident::new("u16", meta.path.span()));
                        } else if meta.path.is_ident("u32") {
                            pvec_type = Some(syn::Ident::new("u32", meta.path.span()));
                        } else if meta.path.is_ident("u64") {
                            pvec_type = Some(syn::Ident::new("u64", meta.path.span()));
                        } else {
                            return Err(meta.error("Expected u8, u16, or u32 for pvec"));
                        }
                        Ok(())
                    }).unwrap();
                }
            } else if path.is_ident("skip_unpack_if") {
                if let syn::Meta::List(list) = &attr.meta {
                    skip_if = Some(list.parse_args::<syn::Expr>().unwrap());
                }
            } else if path.is_ident("pvec_max_preallocated_size") {
                if let syn::Meta::List(list) = &attr.meta {
                    pvec_max_preallocated_size = list.parse_args::<syn::Expr>().unwrap();
                }
            } else if path.is_ident("unpack_vec_len") {
                if let syn::Meta::List(list) = &attr.meta {
                    vec_len = Some(list.parse_args::<syn::Expr>().unwrap());
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
                    if is_cstring {
                        cur = Some(quote::quote! {
                            let #field_name = reader.read_cstring()?;
                            let #field_name = decode_to_string(encoding, (#field_name).as_bytes(), true)?;
                        });
                    } else if let Some(fixed_string) = fixed_string {
                        let trim = syn::LitBool::new(!fstring_no_trim, field.span());
                        cur = Some(quote::quote! {
                            let #field_name = reader.read_fstring(#fixed_string, encoding, #trim)?;
                        });
                    } else if let Some(pstring_type) = pstring_type {
                        cur = Some(quote::quote! {
                            let len = <#pstring_type>::unpack(reader, big, encoding, __info)? as usize;
                            let #field_name = reader.read_exact_vec(len)?;
                            let #field_name = crate::utils::encoding::decode_to_string(encoding, &#field_name, true)?;
                        });
                    }
                }
            }
            if let Some(segment) = type_path.path.segments.first() {
                if segment.ident == "Vec" {
                    if let Some(fixed_vec) = fixed_vec {
                        cur = Some(quote::quote! {
                            let #field_name = reader.read_struct_vec2(#fixed_vec, big, encoding, __info, #pvec_max_preallocated_size)?;
                        });
                    } else if let Some(pvec_type) = pvec_type {
                        cur = Some(quote::quote! {
                            let len = <#pvec_type>::unpack(reader, big, encoding, __info)? as usize;
                            let #field_name = reader.read_struct_vec2(len, big, encoding, __info, #pvec_max_preallocated_size)?;
                        });
                    } else if let Some(vec_len) = vec_len {
                        cur = Some(quote::quote! {
                            let len = (#vec_len) as usize;
                            let #field_name = reader.read_struct_vec2(len, big, encoding, __info, #pvec_max_preallocated_size)?;
                        });
                    }
                }
            }
        }
        let p = cur.unwrap_or_else(|| {
            quote::quote! {
                let #field_name = <#field_type>::unpack(reader, big, encoding, __info)?;
            }
        });
        if let Some(skip_if) = skip_if {
            quote::quote! {
                let #field_name = if !(#skip_if) {
                    #p
                    #field_name
                } else {
                    Default::default()
                };
            }
        } else {
            p
        }
    }).collect();
    let fields = if is_tuple_struct {
        quote::quote! ((#(#fields),*))
    } else {
        quote::quote! { { #(#fields),* } }
    };
    let output = quote::quote! {
        impl StructUnpack for #name {
            fn unpack<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding, __info: &Option<Box<dyn std::any::Any>>) -> Result<Self> {
                #(#smts)*
                Ok(Self #fields)
            }
        }
    };
    output.into()
}

#[cfg(feature = "artemis-arc")]
#[proc_macro]
/// Generates a list of Artemis Arc extensions for the PFS file format.
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

/// A procedural macro for `#[derive(Default)]` that supports a `#[default(expr)]` attribute.
///
/// This macro automatically implements the `Default` trait for a struct or an enum.
/// If a field or enum variant does not have the `#[default(expr)]` attribute, it will
/// use `Default::default()`. If the attribute is present, it will use the specified
/// expression as the default value.
#[proc_macro_derive(Default, attributes(default))]
pub fn default_macro_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let default_body = match &ast.data {
        syn::Data::Struct(data_struct) => {
            // Handle struct fields
            let field_defaults = data_struct.fields.iter().map(|f| {
                let name = &f.ident;
                // Find the `#[default(...)]` attribute
                let default_value = if let Some(default_attr) =
                    f.attrs.iter().find(|attr| attr.path().is_ident("default"))
                {
                    // Parse the expression inside the attribute's parentheses
                    if let Ok(value) = default_attr.parse_args::<syn::Expr>() {
                        quote::quote! { #value }
                    } else {
                        // If parsing fails, panic with a descriptive error
                        panic!("Invalid `#[default]` attribute syntax");
                    }
                } else {
                    // If no `#[default]` attribute is present, fall back to `Default::default()`
                    quote::quote! { Default::default() }
                };
                quote::quote! {
                    #name: #default_value,
                }
            });

            match &data_struct.fields {
                syn::Fields::Named(_) => quote::quote! { Self { #(#field_defaults)* } },
                syn::Fields::Unnamed(_) => quote::quote! { Self(#(#field_defaults)*) },
                syn::Fields::Unit => quote::quote! { Self },
            }
        }
        syn::Data::Enum(data_enum) => {
            // Handle enum variants
            // Find the single variant with the `#[default]` attribute
            if let Some(default_variant) = data_enum
                .variants
                .iter()
                .find(|v| v.attrs.iter().any(|attr| attr.path().is_ident("default")))
            {
                let variant_name = &default_variant.ident;
                match &default_variant.fields {
                    syn::Fields::Unit => quote::quote! { Self::#variant_name },
                    syn::Fields::Unnamed(fields_unnamed) => {
                        let field_defaults = fields_unnamed.unnamed.iter().map(|f| {
                            let default_value = if let Some(default_attr) =
                                f.attrs.iter().find(|attr| attr.path().is_ident("default"))
                            {
                                // Parse the expression inside the attribute's parentheses
                                if let Ok(value) = default_attr.parse_args::<syn::Expr>() {
                                    quote::quote! { #value }
                                } else {
                                    // If parsing fails, panic with a descriptive error
                                    panic!("Invalid `#[default]` attribute syntax");
                                }
                            } else {
                                // If no `#[default]` attribute is present, fall back to `Default::default()`
                                quote::quote! { Default::default() }
                            };
                            quote::quote! { #default_value }
                        });
                        quote::quote! { Self::#variant_name(#(#field_defaults)*) }
                    }
                    syn::Fields::Named(fields_named) => {
                        let field_defaults = fields_named.named.iter().map(|f| {
                            let name = &f.ident;
                            let default_value = if let Some(default_attr) =
                                f.attrs.iter().find(|attr| attr.path().is_ident("default"))
                            {
                                // Parse the expression inside the attribute's parentheses
                                if let Ok(value) = default_attr.parse_args::<syn::Expr>() {
                                    quote::quote! { #value }
                                } else {
                                    // If parsing fails, panic with a descriptive error
                                    panic!("Invalid `#[default]` attribute syntax");
                                }
                            } else {
                                // If no `#[default]` attribute is present, fall back to `Default::default()`
                                quote::quote! { Default::default() }
                            };
                            quote::quote! { #name: #default_value }
                        });
                        quote::quote! { Self::#variant_name { #(#field_defaults)* } }
                    }
                }
            } else {
                // Enums must have exactly one default variant
                panic!("Enum must have one variant with `#[default]` attribute.");
            }
        }
        syn::Data::Union(_) => panic!("`Default` macro cannot be derived for unions"),
    };

    // Construct the final `impl Default for ...` block
    quote::quote! {
        #[automatically_derived]
        impl #impl_generics Default for #name #ty_generics #where_clause {
            fn default() -> Self {
                #default_body
            }
        }
    }
    .into()
}
