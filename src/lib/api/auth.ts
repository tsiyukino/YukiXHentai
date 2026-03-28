import { invoke } from "@tauri-apps/api/core";

export interface LoginResult {
  success: boolean;
  message: string;
}

export async function openLoginWindow(): Promise<void> {
  return invoke("open_login_window");
}

export async function cancelMobileLogin(): Promise<void> {
  return invoke("cancel_mobile_login");
}

export async function login(
  ipb_member_id: string,
  ipb_pass_hash: string,
  igneous: string
): Promise<LoginResult> {
  return invoke("login", { ipbMemberId: ipb_member_id, ipbPassHash: ipb_pass_hash, igneous });
}

export async function logout(): Promise<LoginResult> {
  return invoke("logout");
}

export async function getAuthStatus(): Promise<boolean> {
  return invoke("get_auth_status");
}
