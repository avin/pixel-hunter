import { useEffect, useRef } from "react";
import { Outlet, useParams } from "react-router-dom";
import {
  useEngineActivateMutation,
  useEngineDeactivateMutation,
} from "@/store/api";

/** Keeps the profile's trackers running while any screen of the profile is open. */
export function ProfileRoute() {
  const { profileId = "" } = useParams();
  const [activate] = useEngineActivateMutation();
  const [deactivate] = useEngineDeactivateMutation();
  const epoch = useRef(0);

  useEffect(() => {
    const current = ++epoch.current;
    void activate({ profileId, epoch: current });
    return () => {
      void deactivate({ epoch: current });
    };
  }, [profileId, activate, deactivate]);

  return <Outlet />;
}
