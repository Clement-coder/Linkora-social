import { LinkoraClient } from "../client.js";
import { VersionMismatchError } from "../errors.js";

const mockSimulate = jest.fn();

jest.mock("@stellar/stellar-sdk/rpc", () => {
  const original = jest.requireActual("@stellar/stellar-sdk/rpc");
  return {
    ...original,
    Server: jest.fn().mockImplementation(() => ({
      simulateTransaction: mockSimulate,
    })),
  };
});

describe("Issue #1362: SDK contract version capability check", () => {
  let client: LinkoraClient;

  beforeEach(() => {
    jest.clearAllMocks();
    client = new LinkoraClient({
      contractId: "CDUMMY",
      rpcUrl: "https://rpc.example.com",
    });
  });

  it("returns unknown when contract does not implement version method", async () => {
    mockSimulate.mockResolvedValueOnce({ error: "Method not found" });
    const version = await client.getContractVersion();
    expect(version).toBe("unknown");
  });

  it("returns contract version string when version method succeeds", async () => {
    mockSimulate.mockResolvedValueOnce({
      result: { retval: { _type: "scval", _val: "1.2.0" } },
    });
    const version = await client.getContractVersion();
    expect(version).toBe("1.2.0");
  });

  it("verifies contract version successfully when version matches", async () => {
    mockSimulate.mockResolvedValueOnce({
      result: { retval: { _type: "scval", _val: "1.2.0" } },
    });
    const ok = await client.verifyContractVersion("1.2.0");
    expect(ok).toBe(true);
  });

  it("throws VersionMismatchError when contract version differs", async () => {
    mockSimulate.mockResolvedValueOnce({
      result: { retval: { _type: "scval", _val: "0.9.0" } },
    });
    await expect(client.verifyContractVersion("1.2.0")).rejects.toThrow(VersionMismatchError);
  });
});
