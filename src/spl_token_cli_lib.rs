use domichain_sdk::{
    native_token::*,
    program_option::COption,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};
use spl_token_2022::{
    extension::{
        cpi_guard::CpiGuard,
        default_account_state::DefaultAccountState,
        interest_bearing_mint::InterestBearingConfig,
        memo_transfer::MemoTransfer,
        mint_close_authority::MintCloseAuthority,
        permanent_delegate::PermanentDelegate,
        transfer_fee::{TransferFeeAmount, TransferFeeConfig},
        BaseStateWithExtensions, ExtensionType, StateWithExtensionsOwned,
    },
    instruction::*,
    state::{Account, AccountState, Mint},
};
use std::{collections::HashMap, fmt, fmt::Display, process::exit, str::FromStr, sync::Arc};

fn token_client_from_config(
    config: &Config<'_>,
    token_pubkey: &Pubkey,
    decimals: Option<u8>,
) -> Result<Token<ProgramRpcClientSendTransaction>, Error> {
    let token = Token::new(
        config.program_client.clone(),
        &config.program_id,
        token_pubkey,
        decimals,
        config.fee_payer()?.clone(),
    );

    if let (Some(nonce_account), Some(nonce_authority)) =
        (config.nonce_account, &config.nonce_authority)
    {
        Ok(token.with_nonce(&nonce_account, Arc::clone(nonce_authority)))
    } else {
        Ok(token)
    }
}

#[allow(clippy::too_many_arguments)]
async fn command_create_token(
    config: &Config<'_>,
    decimals: u8,
    token_pubkey: Pubkey,
    authority: Pubkey,
    enable_freeze: bool,
    enable_close: bool,
    enable_non_transferable: bool,
    enable_permanent_delegate: bool,
    memo: Option<String>,
    rate_bps: Option<i16>,
    default_account_state: Option<AccountState>,
    transfer_fee: Option<(u16, u64)>,
    bulk_signers: Vec<Arc<dyn Signer>>,
) -> CommandResult {
    println_display(
        config,
        format!(
            "Creating token {} under program {}",
            token_pubkey, config.program_id
        ),
    );

    let token = token_client_from_config(config, &token_pubkey, Some(decimals))?;

    let freeze_authority = if enable_freeze { Some(authority) } else { None };

    let mut extensions = vec![];

    if enable_close {
        extensions.push(ExtensionInitializationParams::MintCloseAuthority {
            close_authority: Some(authority),
        });
    }

    if enable_permanent_delegate {
        extensions.push(ExtensionInitializationParams::PermanentDelegate {
            delegate: authority,
        });
    }

    if let Some(rate_bps) = rate_bps {
        extensions.push(ExtensionInitializationParams::InterestBearingConfig {
            rate_authority: Some(authority),
            rate: rate_bps,
        })
    }

    if enable_non_transferable {
        extensions.push(ExtensionInitializationParams::NonTransferable);
    }

    if let Some(state) = default_account_state {
        assert!(
            enable_freeze,
            "Token requires a freeze authority to default to frozen accounts"
        );
        extensions.push(ExtensionInitializationParams::DefaultAccountState { state })
    }

    if let Some((transfer_fee_basis_points, maximum_fee)) = transfer_fee {
        extensions.push(ExtensionInitializationParams::TransferFeeConfig {
            transfer_fee_config_authority: Some(authority),
            withdraw_withheld_authority: Some(authority),
            transfer_fee_basis_points,
            maximum_fee,
        });
    }

    if let Some(text) = memo {
        token.with_memo(text, vec![config.default_signer()?.pubkey()]);
    }

    let res = token
        .create_mint(
            &authority,
            freeze_authority.as_ref(),
            extensions,
            &bulk_signers,
        )
        .await?;

    let tx_return = finish_tx(config, &res, false).await?;
    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliCreateToken {
                address: token_pubkey.to_string(),
                decimals,
                transaction_data: cli_signature,
            },
            &CommandName::CreateToken,
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, &CommandName::CreateToken, config)
        }
    })
}

fn new_throwaway_signer() -> (Arc<dyn Signer>, Pubkey) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    (Arc::new(keypair) as Arc<dyn Signer>, pubkey)
}

/// Copy of: domichain-program-library/token/cli/src/main.rs
pub async fn main(
    decimals: u8,
    mint_authority: Pubkey,
    memo: Option<String>,
    interest_rate: Option<i16>,
    transfer_fee: Option<(u16, u64)>,
    (token_signer, token): (Arc<dyn Signer>, Pubkey),
    default_account_state: AccountState,
) {
    // CommandName::CreateToken
    let decimals = value_t_or_exit!(arg_matches, "decimals", u8);
    let mint_authority =
        config.pubkey_or_default(arg_matches, "mint_authority", &mut wallet_manager)?;
    let memo = value_t!(arg_matches, "memo", String).ok();
    let rate_bps = value_t!(arg_matches, "interest_rate", i16).ok();

    let transfer_fee = arg_matches.values_of("transfer_fee").map(|mut v| {
        (
            v.next()
                .unwrap()
                .parse::<u16>()
                .unwrap_or_else(print_error_and_exit),
            v.next()
                .unwrap()
                .parse::<u64>()
                .unwrap_or_else(print_error_and_exit),
        )
    });

    let (token_signer, token) =
        get_signer(arg_matches, "token_keypair", &mut wallet_manager)
            .unwrap_or_else(new_throwaway_signer);
    push_signer_with_dedup(token_signer, &mut bulk_signers);
    let default_account_state =
        arg_matches
            .value_of("default_account_state")
            .map(|s| match s {
                "initialized" => AccountState::Initialized,
                "frozen" => AccountState::Frozen,
                _ => unreachable!(),
            });

    command_create_token(
        config,
        decimals,
        token,
        mint_authority,
        arg_matches.is_present("enable_freeze"),
        arg_matches.is_present("enable_close"),
        arg_matches.is_present("enable_non_transferable"),
        arg_matches.is_present("enable_permanent_delegate"),
        memo,
        rate_bps,
        default_account_state,
        transfer_fee,
        bulk_signers,
    )
    .await
}