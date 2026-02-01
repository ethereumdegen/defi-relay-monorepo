// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title PermitExecutor
/// @notice Executes EIP-2612 permit + transferFrom atomically
/// @dev The spender in permits must be this contract's address, not the facilitator EOA
contract PermitExecutor {
    address public immutable owner;

    error NotOwner();
    error PermitFailed();
    error TransferFailed();
    error DeploymentFailed();

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor(address _owner) {
        owner = _owner;
    }

    /// @notice Execute permit + transferFrom atomically using split signature (v, r, s)
    /// @param token The ERC20 token with EIP-2612 permit support
    /// @param tokenOwner The owner of the tokens (signer of the permit)
    /// @param value Amount of tokens to transfer
    /// @param deadline Permit deadline timestamp
    /// @param v Signature v component
    /// @param r Signature r component
    /// @param s Signature s component
    /// @param payTo Recipient of the tokens
    function executePermitTransfer(
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        uint8 v,
        bytes32 r,
        bytes32 s,
        address payTo
    ) external onlyOwner {
        _permit(token, tokenOwner, value, deadline, v, r, s);
        _transferFrom(token, tokenOwner, payTo, value);
    }

    /// @notice Execute permit + transferFrom atomically using bytes signature (EIP-1271 compatible)
    /// @param token The ERC20 token with EIP-2612 permit support
    /// @param tokenOwner The owner of the tokens (signer of the permit)
    /// @param value Amount of tokens to transfer
    /// @param deadline Permit deadline timestamp
    /// @param signature Full signature bytes
    /// @param payTo Recipient of the tokens
    function executePermitTransferWithSignature(
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        bytes calldata signature,
        address payTo
    ) external onlyOwner {
        _permitWithSignature(token, tokenOwner, value, deadline, signature);
        _transferFrom(token, tokenOwner, payTo, value);
    }

    /// @notice Execute counterfactual wallet deployment + permit + transferFrom (EIP-6492)
    /// @param factory The wallet factory address
    /// @param factoryCalldata Calldata to deploy the wallet
    /// @param token The ERC20 token with EIP-2612 permit support
    /// @param tokenOwner The owner of the tokens (counterfactual wallet address)
    /// @param value Amount of tokens to transfer
    /// @param deadline Permit deadline timestamp
    /// @param signature Inner signature bytes (unwrapped from EIP-6492)
    /// @param payTo Recipient of the tokens
    function executeCounterfactualPermitTransfer(
        address factory,
        bytes calldata factoryCalldata,
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        bytes calldata signature,
        address payTo
    ) external onlyOwner {
        // Deploy wallet if not already deployed (allowFailure - may already exist)
        if (tokenOwner.code.length == 0) {
            (bool deploySuccess, ) = factory.call(factoryCalldata);
            // Only fail if deployment failed AND wallet still doesn't exist
            if (!deploySuccess && tokenOwner.code.length == 0) {
                revert DeploymentFailed();
            }
        }

        _permitWithSignature(token, tokenOwner, value, deadline, signature);
        _transferFrom(token, tokenOwner, payTo, value);
    }

    /// @notice Execute counterfactual wallet deployment + permit + transferFrom with split sig (EIP-6492)
    /// @param factory The wallet factory address
    /// @param factoryCalldata Calldata to deploy the wallet
    /// @param token The ERC20 token with EIP-2612 permit support
    /// @param tokenOwner The owner of the tokens (counterfactual wallet address)
    /// @param value Amount of tokens to transfer
    /// @param deadline Permit deadline timestamp
    /// @param v Signature v component
    /// @param r Signature r component
    /// @param s Signature s component
    /// @param payTo Recipient of the tokens
    function executeCounterfactualPermitTransferSplit(
        address factory,
        bytes calldata factoryCalldata,
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        uint8 v,
        bytes32 r,
        bytes32 s,
        address payTo
    ) external onlyOwner {
        // Deploy wallet if not already deployed
        if (tokenOwner.code.length == 0) {
            (bool deploySuccess, ) = factory.call(factoryCalldata);
            if (!deploySuccess && tokenOwner.code.length == 0) {
                revert DeploymentFailed();
            }
        }

        _permit(token, tokenOwner, value, deadline, v, r, s);
        _transferFrom(token, tokenOwner, payTo, value);
    }

    // ============ Internal Functions ============

    function _permit(
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) internal {
        (bool success, ) = token.call(
            abi.encodeWithSignature(
                "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)",
                tokenOwner,
                address(this),
                value,
                deadline,
                v,
                r,
                s
            )
        );
        if (!success) revert PermitFailed();
    }

    function _permitWithSignature(
        address token,
        address tokenOwner,
        uint256 value,
        uint256 deadline,
        bytes calldata signature
    ) internal {
        (bool success, ) = token.call(
            abi.encodeWithSignature(
                "permit(address,address,uint256,uint256,bytes)",
                tokenOwner,
                address(this),
                value,
                deadline,
                signature
            )
        );
        if (!success) revert PermitFailed();
    }

    function _transferFrom(
        address token,
        address from,
        address to,
        uint256 value
    ) internal {
        (bool success, ) = token.call(
            abi.encodeWithSignature(
                "transferFrom(address,address,uint256)",
                from,
                to,
                value
            )
        );
        if (!success) revert TransferFailed();
    }
}
