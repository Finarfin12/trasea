import { useState } from "react";
import { downloadAndLaunchUpdate, openExternalUrl, type UpdateInfo } from "./api";
import { useI18n } from "./i18n";

let activeDownload: ReturnType<typeof downloadAndLaunchUpdate> | null = null;

export function UpdateActions({ info }: { info: UpdateInfo }) {
	const { t } = useI18n();
	const [busy, setBusy] = useState(false);
	const [status, setStatus] = useState("");
	const [error, setError] = useState(false);

	async function download() {
		setBusy(true);
		setError(false);
		setStatus(t.updateDownloading);
		try {
			activeDownload ??= downloadAndLaunchUpdate(info).finally(() => { activeDownload = null; });
			const result = await activeDownload;
			setStatus(`${result.sha256Verified ? t.updateVerified : t.updateNoChecksum} ${result.launched ? t.updateOpened : t.updateSaved} ${result.savedPath}`);
		} catch (e) {
			setError(true);
			setStatus(`${t.updateFailed} ${String(e)}`);
		} finally {
			setBusy(false);
		}
	}

	return (
		<div className="update-actions">
			{info.assetUrl && info.assetName && (
				<>
					<small>{info.assetName}{info.assetSize ? ` · ${(info.assetSize / 1024 ** 2).toFixed(1)} MiB` : ""}</small>
					<button className="small primary" disabled={busy} onClick={() => void download()}>
						{info.isPortable ? t.updatePortable : t.updateInstall}
					</button>
				</>
			)}
			<button className="small" onClick={() => void openExternalUrl(info.downloadUrl).catch(e => {
				setError(true);
				setStatus(String(e));
			})}>{t.updateGitHub}</button>
			{status && <small role={error ? "alert" : "status"}>{status}</small>}
		</div>
	);
}
