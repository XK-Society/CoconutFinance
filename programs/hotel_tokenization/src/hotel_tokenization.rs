use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint as AnchorMint, TokenAccount, TokenInterface};
use anchor_spl::associated_token::AssociatedToken;
use spl_token_2022::state::Mint as SplMint;
use spl_token_2022::extension::{BaseStateWithExtensions, transfer_fee};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod hotel_tokenization {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, room_count: u64, transfer_fee_basis_points: u16) -> Result<()> {
        let hotel = &mut ctx.accounts.hotel;
        hotel.authority = ctx.accounts.authority.key();
        hotel.room_count = room_count;
        hotel.rooms_minted = 0;
        hotel.total_profit = 0;

        // Initialize transfer fee extension
        let mint_info = ctx.accounts.room_mint.to_account_info();
        let mint = BaseStateWithExtensions::<SplMint>::unpack(&mint_info.data.borrow())?;
        let mut transfer_fee_config = mint.get_extension_mut::<transfer_fee::TransferFeeConfig>()?;
        transfer_fee_config.transfer_fee_basis_points = transfer_fee_basis_points;
        transfer_fee_config.maximum_fee = u64::MAX; // Set an appropriate maximum fee

        Ok(())
    }

    pub fn mint_room_token(ctx: Context<MintRoomToken>, room_number: u64) -> Result<()> {
        let hotel = &mut ctx.accounts.hotel;
        require!(room_number <= hotel.room_count, ErrorCode::InvalidRoomNumber);
        require!(hotel.rooms_minted < hotel.room_count, ErrorCode::AllRoomsMinted);

        // Mint 1 token to represent the room
        anchor_spl::token_2022::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::MintTo {
                    mint: ctx.accounts.room_mint.to_account_info(),
                    to: ctx.accounts.user_room_ata.to_account_info(),
                    authority: ctx.accounts.hotel.to_account_info(),
                },
            ),
            1,
        )?;

        hotel.rooms_minted += 1;
        Ok(())
    }

    pub fn book_room(ctx: Context<BookRoom>, room_number: u64, booking_price: u64) -> Result<()> {
        let hotel = &mut ctx.accounts.hotel;
        require!(room_number <= hotel.room_count, ErrorCode::InvalidRoomNumber);

        // Transfer USDC from tourist to hotel vault
        anchor_spl::token_2022::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::Transfer {
                    from: ctx.accounts.tourist_usdc_account.to_account_info(),
                    to: ctx.accounts.hotel_usdc_vault.to_account_info(),
                    authority: ctx.accounts.tourist.to_account_info(),
                },
            ),
            booking_price,
        )?;

        hotel.total_profit += booking_price;

        // Emit a booking event
        emit!(BookingEvent {
            room_number,
            tourist: ctx.accounts.tourist.key(),
            price: booking_price,
        });

        Ok(())
    }

    pub fn distribute_profits(ctx: Context<DistributeProfits>) -> Result<()> {
        let hotel = &mut ctx.accounts.hotel;
        let total_profit = hotel.total_profit;
        require!(total_profit > 0, ErrorCode::NoProfitToDistribute);

        let total_supply = ctx.accounts.room_mint.supply;
        let profit_per_token = total_profit / total_supply;

        let user_token_balance = ctx.accounts.user_room_ata.amount;
        let user_profit = profit_per_token * user_token_balance;

        // Transfer profit in USDC to the user
        anchor_spl::token_2022::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::Transfer {
                    from: ctx.accounts.hotel_usdc_vault.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: ctx.accounts.hotel.to_account_info(),
                },
                &[&[b"hotel", &[*ctx.bumps.get("hotel").unwrap()]]],
            ),
            user_profit,
        )?;

        hotel.total_profit -= user_profit;

        // Emit a profit distribution event
        emit!(ProfitDistributionEvent {
            user: ctx.accounts.user.key(),
            amount: user_profit,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + 32 + 8 + 8 + 8)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: This is the token-2022 program
    #[account(address = spl_token_2022::id())]
    pub token_program: AccountInfo<'info>,
    #[account(
        init,
        payer = authority,
        mint::decimals = 0,
        mint::authority = hotel,
        mint::freeze_authority = hotel,
        token::token_program = token_program,
    )]
    pub room_mint: InterfaceAccount<'info, AnchorMint>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintRoomToken<'info> {
    #[account(mut)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub room_mint: InterfaceAccount<'info, AnchorMint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = room_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_room_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BookRoom<'info> {
    #[account(mut)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub tourist: Signer<'info>,
    #[account(mut)]
    pub tourist_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub hotel_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct DistributeProfits<'info> {
    #[account(mut)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub room_mint: InterfaceAccount<'info, AnchorMint>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_room_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub hotel_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct Hotel {
    pub authority: Pubkey,
    pub room_count: u64,
    pub rooms_minted: u64,
    pub total_profit: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid room number")]
    InvalidRoomNumber,
    #[msg("All rooms have been minted")]
    AllRoomsMinted,
    #[msg("No profit to distribute")]
    NoProfitToDistribute,
}

#[event]
pub struct BookingEvent {
    pub room_number: u64,
    pub tourist: Pubkey,
    pub price: u64,
}

#[event]
pub struct ProfitDistributionEvent {
    pub user: Pubkey,
    pub amount: u64,
}