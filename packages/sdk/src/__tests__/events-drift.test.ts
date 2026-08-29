import { parseContractEvent } from "../events/types.js";
import { parseRawContractEvent } from "../generated/events.js";

describe("Event type drift", () => {
  it("dummy runtime check to satisfy jest", () => {
    expect(typeof parseContractEvent).toBe("function");
    expect(typeof parseRawContractEvent).toBe("function");
  });
});
