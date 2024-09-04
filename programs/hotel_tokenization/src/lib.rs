use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub mod hotel_tokenization;
pub mod liquidity_pool;

use hotel_tokenization::*;
use liquidity_pool::*;

#[program]
pub mod hotel_token_program {
    use super::*;

    // Hotel Tokenization Instructions
    pub fn initialize_hotel(ctx: Context<hotel_tokenization::Initialize>, room_count: u64, transfer_fee_basis_points: u16) -> Result<()> {
        hotel_tokenization::initialize(ctx, room_count, transfer_fee_basis_points)
    }

    pub fn mint_room_token(ctx: Context<hotel_tokenization::MintRoomToken>, room_number: u64) -> Result<()> {
        hotel_tokenization::mint_room_token(ctx, room_number)
    }

    pub fn book_room(ctx: Context<hotel_tokenization::BookRoom>, room_number: u64, booking_price: u64) -> Result<()> {
        hotel_tokenization::book_room(ctx, room_number, booking_price)
    }

    pub fn distribute_profits(ctx: Context<hotel_tokenization::DistributeProfits>) -> Result<()> {
        hotel_tokenization::distribute_profits(ctx)
    }

    // Liquidity Pool Instructions
    pub fn initialize_pool(ctx: Context<liquidity_pool::Initialize>, fee_basis_points: u16) -> Result<()> {
        liquidity_pool::initialize(ctx, fee_basis_points)
    }

    pub fn provide_liquidity(ctx: Context<liquidity_pool::ProvideLiquidity>, usdc_amount: u64) -> Result<()> {
        liquidity_pool::provide_liquidity(ctx, usdc_amount)
    }

    pub fn withdraw_liquidity(ctx: Context<liquidity_pool::WithdrawLiquidity>, lp_token_amount: u64) -> Result<()> {
        liquidity_pool::withdraw_liquidity(ctx, lp_token_amount)
    }
}