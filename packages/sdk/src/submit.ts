import * as rpc from "@stellar/stellar-sdk/rpc";
import { Transaction } from "@stellar/stellar-base";
import {
  TransactionQueue,
  QueueSigner,
  RunOptions,
  RpcClient,
  SimulationResult,
} from "./queue.js";
import type { LinkoraClient } from "./client.js";

/**
 * Adapt a Soroban-rpc `Server` to the narrower {@link RpcClient} interface that
 * `TransactionQueue` consumes, translating the raw SDK response shapes into the
 * simplified ones the queue expects. Without this, the native `rpc.Server`
 * (whose `simulateTransaction`/`sendTransaction`/`getTransaction` return rich,
 * differently-shaped responses) is not structurally assignable to `RpcClient`.
 */
export function serverToRpcClient(server: rpc.Server): RpcClient {
  return {
    async simulateTransaction(xdr: string): Promise<SimulationResult> {
      const response = await server.simulateTransaction(xdr);
      if (rpc.Api.isSimulationError(response)) {
        return { success: false, resourceFee: "0", error: response.error };
      }
      return { success: true, resourceFee: response.minResourceFee || "0" };
    },
    async sendTransaction(signedXdr: string) {
      const response = await server.sendTransaction(signedXdr);
      return {
        hash: response.hash,
        status: response.status,
        errorResultXdr: response.errorResultXdr,
      };
    },
    async getTransaction(hash: string) {
      const response = await server.getTransaction(hash);
      return { status: response.status, errorResultXdr: response.errorResultXdr };
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
    rpc: serverToRpcClient(client.createRpcServer()),
  });

  queue.enqueue(xdrString);
  await queue.run(opts);
  
  const hashes = queue.submittedHashes;
  if (hashes.length === 0 && !opts?.dryRun) {
    throw new Error("Transaction was not submitted successfully.");
  }
  
  return hashes[0] ?? "";
}
