import {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import '@saurl/tauri-plugin-safe-area-insets-css-api';
import "./App.css";

invoke("discover").then();

interface Device {
    name: string;
    address: string;
}

type Response = { type: "Event", payload: unknown } | { type: "Response", uuid: string, result: unknown };
type Event = {
    type: "DeviceListChanged",
    data: { items: Device[] }
}

function App() {
    const [devices, setDevices] = useState<Device[]>([]);

    useEffect(() => {
        invoke("get_nearby_devices").then(result => {
            setDevices(result as Device[]);
        })
    }, []);

    useEffect(() => {
        const socket = new WebSocket("ws://localhost:50051/ws");

        socket.onopen = () => {
            console.log("connected");
            socket.send("Hello server");
        };

        socket.onmessage = ({data}) => {
            console.log(typeof data, JSON.parse(data))

            const response = JSON.parse(data) as Response;
            if (response.type == "Event") {
                const payload = response.payload as Event;

                if (payload.type == "DeviceListChanged") {
                    setDevices(payload.data.items);
                }
            }
        };

        socket.onerror = (err) => {
            console.error("WebSocket error:", err);
        };

        socket.onclose = () => {
            console.log("disconnected");
        };

        return () => {
            socket.close();
        };
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
