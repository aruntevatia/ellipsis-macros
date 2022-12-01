//! These macros are used cede generation in Solana smart contracts
//!
//! Currently, the implemented macros can generate static program IDs
//! and deterministic program derived addresses (the bump seed is autogenerated).
//!
//! The code is forked from the Solana SDK (https://github.com/solana-labs/solana/blob/master/sdk/macro/src/lib.rs)
//! and modified to support new features.
//!

use std::str::FromStr;

extern crate proc_macro;

use {
    proc_macro::TokenStream,
    proc_macro2::Span,
    quote::{quote, ToTokens},
    solana_program::pubkey::Pubkey,
    std::convert::TryFrom,
    syn::{
        parse::{Parse, ParseStream, Result},
        parse_macro_input, Expr, LitByte, LitStr, Token,
    },
};

fn parse_id(input: ParseStream) -> Result<proc_macro2::TokenStream> {
    let id = if input.peek(syn::LitStr) {
        let id_literal: LitStr = input.parse()?;
        parse_pubkey(&id_literal)?
    } else {
        let expr: Expr = input.parse()?;
        quote! { #expr }
    };

    if !input.is_empty() {
        let stream: proc_macro2::TokenStream = input.parse()?;
        return Err(syn::Error::new_spanned(stream, "unexpected token"));
    }
    Ok(id)
}

fn parse_pubkey(id_literal: &LitStr) -> Result<proc_macro2::TokenStream> {
    let id_vec = bs58::decode(id_literal.value())
        .into_vec()
        .map_err(|_| syn::Error::new_spanned(id_literal, "failed to decode base58 string"))?;
    let id_array = <[u8; 32]>::try_from(<&[u8]>::clone(&&id_vec[..])).map_err(|_| {
        syn::Error::new_spanned(
            id_literal,
            format!("pubkey array is not 32 bytes long: len={}", id_vec.len()),
        )
    })?;
    let bytes = id_array.iter().map(|b| LitByte::new(*b, Span::call_site()));
    Ok(quote! {
        ::solana_program::pubkey::Pubkey::new_from_array(
            [#(#bytes,)*]
        )
    })
}

fn parse_pda(
    id_literal: &LitStr,
    program_id: &LitStr,
    seed: &LitStr,
) -> Result<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let pda_key = Pubkey::from_str(&id_literal.value())
        .map_err(|_| syn::Error::new_spanned(id_literal, "failed to decode base58 string"))?;
    let program_id = Pubkey::from_str(&program_id.value())
        .map_err(|_| syn::Error::new_spanned(id_literal, "failed to decode base58 string"))?;

    let (computed_key, bump_seed) =
        Pubkey::find_program_address(&[&seed.value().as_ref()], &program_id);

    if pda_key != computed_key {
        return Err(syn::Error::new_spanned(
            id_literal,
            "provided PDA does not match the computed PDA",
        ));
    }

    let pda_token_stream = parse_pubkey(id_literal)?;

    let bump = LitByte::new(bump_seed, Span::call_site());
    let bump_token_stream = quote! {
        #bump
    };
    Ok((pda_token_stream, bump_token_stream))
}

fn generate_static_pubkey_code(
    id: &proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
) {
    tokens.extend(quote! {
        /// The static program ID
        pub static ID: ::solana_program::pubkey::Pubkey = #id;

        /// Confirms that a given pubkey is equivalent to the program ID
        pub fn check_id(id: &::solana_program::pubkey::Pubkey) -> bool {
            id == &ID
        }

        /// Returns the program ID
        pub fn id() -> ::solana_program::pubkey::Pubkey {
            ID
        }

        #[cfg(test)]
        #[test]
        fn test_id() {
            assert!(check_id(&id()));
        }
    });
}

fn generate_static_bump_code(
    bump: &proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
) {
    tokens.extend(quote! {
        /// The bump seed of the static PDA
        pub const BUMP: u8 = #bump;
    });
}

struct Id(proc_macro2::TokenStream);

impl Parse for Id {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_id(input).map(Self)
    }
}

impl ToTokens for Id {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        generate_static_pubkey_code(&self.0, tokens)
    }
}

struct ProgramPdaArgs {
    pda: proc_macro2::TokenStream,
    bump: proc_macro2::TokenStream,
}

impl Parse for ProgramPdaArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let pda_address: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let program_id: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let seed: LitStr = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("unexpected token"));
        }
        let (pda, bump) = parse_pda(&pda_address, &program_id, &seed)?;
        Ok(Self { pda, bump })
    }
}

impl ToTokens for ProgramPdaArgs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        generate_static_bump_code(&self.bump, tokens);
        generate_static_pubkey_code(&self.pda, tokens)
    }
}

#[proc_macro]
pub fn declare_id(input: TokenStream) -> TokenStream {
    let id = parse_macro_input!(input as Id);
    TokenStream::from(quote! {#id})
}

#[proc_macro]
pub fn declare_pda(input: TokenStream) -> TokenStream {
    let id = parse_macro_input!(input as ProgramPdaArgs);
    TokenStream::from(quote! {#id})
}