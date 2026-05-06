import {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import '@saurl/tauri-plugin-safe-area-insets-css-api';
import "./App.css";
import {listen} from "@tauri-apps/api/event";

invoke("discover").then();

interface Device {
    name: string;
    address: string;
}

function App() {
    const [devices, setDevices] = useState<Device[]>([]);

    useEffect(() => {
        const ev = listen<Device[]>('device-list-changed', (event) => {
            setDevices(event.payload)
        });
        invoke("discover");

        return () => {
            invoke("cancel_discovery")
            ev.then(unlisten => unlisten());
        }
    }, []);

    return (
        <main>
            <h1>Welcome to Tauri + React</h1>

            <ul>
                {devices.map((device) => (
                    <li key={device.name}>
                        {device.name}: {device.address}
                    </li>
                ))}
            </ul>
        </main>
    );
}

export default App;
