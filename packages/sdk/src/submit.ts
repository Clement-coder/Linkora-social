import { Transaction } from "@stellar/stellar-base";
import { rpc } from "@stellar/stellar-sdk";
import { TransactionQueue, QueueSigner, RunOptions, RpcClient, SimulationResult } from "./queue.js";
import type { LinkoraClient } from "./client.js";

const { isSimulationError } = rpc.Api;

/**
 * Wraps a Stellar SDK `rpc.Server` to satisfy the {@link RpcClient} interface
 * expected by {@link TransactionQueue}.  The SDK's `Server` has richer
 * method signatures (accepting Transaction objects, extra params, etc.) while
 * `RpcClient` is a narrow, XDR-string-only contract used internally by the
 * queue.
 */
function createRpcAdapter(server: rpc.Server): RpcClient {
  return {
    async simulateTransaction(xdr: string): Promise<SimulationResult> {
      // The SDK's simulateTransaction expects a Transaction object.  Build a
      // minimal Transaction from the XDR string so the call succeeds.
      const { TransactionBuilder } = await import("@stellar/stellar-base");
      // Use a dummy passphrase – the simulation endpoint doesn't validate it.
      const tx = TransactionBuilder.fromXDR(xdr, "Test SDF Network ; September 2015");
      const result = await server.simulateTransaction(tx);
      const isError = isSimulationError(result);
      return {
        success: !isError,
        resourceFee: String("minResourceFee" in result ? result.minResourceFee : "0"),
        error: isError ? result.error : undefined,
      };
    },

    async sendTransaction(signedXdr: string) {
      const { TransactionBuilder } = await import("@stellar/stellar-base");
      const tx = TransactionBuilder.fromXDR(signedXdr, "Test SDF Network ; September 2015");
      const result = await server.sendTransaction(tx);
      return {
        hash: result.hash,
        status: result.status as string,
        errorResultXdr: "errorResultXdr" in result ? String(result.errorResultXdr) : undefined,
      };
    },

    async getTransaction(hash: string) {
      const result = await server.getTransaction(hash);
      return {
        status: result.status as string,
        errorResultXdr: "errorResultXdr" in result ? String(result.errorResultXdr) : undefined,
      };
    },
  };
}

/**
 * Convenience helper to sign and submit a single transaction.
 * Internally sets up a TransactionQueue, enqueues the transaction, and runs it.
 *
 * @param client The LinkoraClient instance used for RPC communication.
 * @param xdrOrTx The transaction to submit (base64 XDR string or Transaction object).
 * @param signer The wallet signer (e.g. FreighterSigner or LedgerSigner).
 * @param opts Optional RunOptions for the queue.
 * @returns The hash of the submitted transaction.
 */
export async function submitTransaction(
  client: LinkoraClient,
  xdrOrTx: string | Transaction,
  signer: QueueSigner,
  opts?: RunOptions
): Promise<string> {
  const xdrString = typeof xdrOrTx === "string" ? xdrOrTx : xdrOrTx.toEnvelope().toXDR("base64");

  const queue = new TransactionQueue({
    signer,
    rpc: createRpcAdapter(client.createRpcServer()),
  });

  queue.enqueue(xdrString);
  await queue.run(opts);

  const hashes = queue.submittedHashes;
  if (hashes.length === 0 && !opts?.dryRun) {
    throw new Error("Transaction was not submitted successfully.");
  }

  return hashes[0] ?? "";
}
