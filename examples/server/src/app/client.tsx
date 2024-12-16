"use client";
import { useEffect } from "react";

const Client = () => {
    useEffect(() => {
        console.log("Client");
    }, []);

    return <div>hello</div>;
};

export default Client;
