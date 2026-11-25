import { useState, type MouseEvent } from "react";
import { GithubLogoIcon } from "@phosphor-icons/react/dist/csr/GithubLogo";
import s from "./DiscordLink.module.css";

const links = [
  { label: "Repositorio de Panas RPG Launcher en GitHub", short: "GitHub", url: "https://github.com/rongorus1/LauncherPanasRPG", icon: <GithubLogoIcon size={22} weight="fill" aria-hidden /> },
  { label: "Modpack Panas RPG en CurseForge", short: "CurseForge", url: "https://www.curseforge.com/minecraft/modpacks/panasrpg-v-1", icon: <span className={s.cfIcon} aria-hidden>CF</span> },
];

export function ProjectLinks() {
  const [failed, setFailed] = useState("");
  async function open(event: MouseEvent<HTMLAnchorElement>, url: string, label: string) {
    if (!("__TAURI_INTERNALS__" in window)) return;
    event.preventDefault();
    setFailed("");
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch { setFailed(label); }
  }
  return <>
    {links.map((link) => <a key={link.short} className={s.link} href={link.url} target="_blank" rel="noopener noreferrer" title={link.label} aria-label={`Abrir ${link.label}`} onClick={event => void open(event, link.url, link.short)}>
      {link.icon}
    </a>)}
    {failed && <span role="alert" className={s.error}>No se pudo abrir {failed}. <a href={links.find(x => x.short === failed)?.url}>{links.find(x => x.short === failed)?.url}</a><button type="button" onClick={() => setFailed("")}>Cerrar</button></span>}
  </>;
}
