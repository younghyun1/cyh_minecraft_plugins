// Generated from rust-be-template OpenAPI. Do not edit by hand.

export type ApiTransport = (
  path: string,
  init: RequestInit,
) => Promise<Response>;

export type ApiRequestOptions = {
  readonly headers?: HeadersInit;
  readonly signal?: AbortSignal;
};

export class ApiContractError extends Error {
  readonly status: number;
  readonly body: string;

  constructor(status: number, body: string) {
    super(contractErrorMessage(status, body));
    this.name = "ApiContractError";
    this.status = status;
    this.body = body;
  }
}

function contractErrorMessage(status: number, body: string): string {
  if (!body) return `Request failed with status ${status}`;
  try {
    const parsed: unknown = JSON.parse(body);
    if (typeof parsed === "object" && parsed !== null && "message" in parsed) {
      const message = (parsed as { readonly message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message.trim();
    }
  } catch {
    return body;
  }
  return body;
}

export type ParameterValue = string | number | boolean;
export type QueryParameterValue = ParameterValue | null;

export function interpolatePath(
  template: string,
  values: Readonly<Record<string, ParameterValue>>,
): string {
  return template.replaceAll(/{([^}]+)}/g, (_match, key: string) =>
    encodeURIComponent(String(values[key])),
  );
}

export function appendQuery(
  path: string,
  values: Readonly<Record<string, QueryParameterValue | undefined>> | undefined,
): string {
  if (values === undefined) return path;
  const query = new URLSearchParams();
  for (const [name, value] of Object.entries(values)) {
    if (value !== undefined && value !== null) query.append(name, String(value));
  }
  const encoded = query.toString();
  return encoded ? `${path}?${encoded}` : path;
}

export function requestHeaders(
  headers: HeadersInit | undefined,
  json: boolean,
): Headers {
  const result = new Headers(headers);
  if (json && !result.has("content-type")) {
    result.set("content-type", "application/json");
  }
  return result;
}

async function requireSuccess(response: Response): Promise<Response> {
  if (response.ok) return response;
  throw new ApiContractError(response.status, await response.text());
}

export async function requestJson<T>(
  transport: ApiTransport,
  path: string,
  init: RequestInit,
): Promise<T> {
  const response = await requireSuccess(await transport(path, init));
  return (await response.json()) as T;
}

export async function requestText<T>(
  transport: ApiTransport,
  path: string,
  init: RequestInit,
): Promise<T> {
  const response = await requireSuccess(await transport(path, init));
  return (await response.text()) as unknown as T;
}

